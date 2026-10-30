//! Counter guards: a headless window's allocations, ECS counts and sprite
//! batches, pinned per guard in a JSON file, so a change that adds work to
//! a frame fails a test instead of waiting for a sitting.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::Harness;
use crate::LogConsole;
use crate::app::FrameExtract;

/// The variable that makes [`CounterGuard::check`] rewrite its pin.
const WRITE_VAR: &str = "TUNGSTEN_GUARDS_WRITE";

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
    static ALLOCATED_BYTES: Cell<u64> = const { Cell::new(0) };
}

/// A global allocator over [`System`] that counts each thread's
/// allocations in thread-local cells, which never allocate. A test binary
/// installs it, and [`allocations`] reads the calling thread's counts:
///
/// ```ignore
/// #[global_allocator]
/// static ALLOC: tungsten::testing::CountingAlloc = tungsten::testing::CountingAlloc;
/// ```
///
/// Its counters are the one global state the engine keeps, and only in
/// test binaries; no shipped build installs it.
pub struct CountingAlloc;

// SAFETY: every call forwards to `System` unchanged; counting touches only
// const-initialized thread-local cells, which do not allocate.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller's contract is `System.alloc`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller's contract is `System.alloc_zeroed`'s.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract is `System.dealloc`'s.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: the caller's contract is `System.realloc`'s.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Counts one allocation of `bytes`; a thread past its teardown has no
/// cells left and counts nothing.
fn record(bytes: usize) {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    let _ = ALLOCATED_BYTES.try_with(|total| total.set(total.get() + bytes as u64));
}

/// The calling thread's allocation totals under [`CountingAlloc`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Allocations {
    /// Allocations and reallocations made.
    pub count: u64,
    /// Bytes they asked for: a reallocation counts its new size.
    pub bytes: u64,
}

/// The calling thread's allocations since it started; zero when the binary
/// has not installed [`CountingAlloc`].
#[must_use]
pub fn allocations() -> Allocations {
    Allocations {
        count: ALLOCATIONS.with(Cell::get),
        bytes: ALLOCATED_BYTES.with(Cell::get),
    }
}

/// One window's counts from [`measure`](Self::measure): what the frames
/// allocated on the test's thread, what the World did, and the most sprite
/// batches and instances one frame extracted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CounterGuard {
    /// Allocations and reallocations, [`CountingAlloc`] installed.
    pub allocations: u64,
    /// The bytes those asked for.
    pub allocated_bytes: u64,
    /// [`WorldCounters::archetype_moves`](tungsten_core::WorldCounters::archetype_moves).
    pub archetype_moves: u64,
    /// Entities spawned.
    pub spawned: u64,
    /// Entities despawned.
    pub despawned: u64,
    /// Events the window's flushes counted.
    pub events_sent: u64,
    /// The most sprite batches one frame extracted.
    pub max_sprite_batches: u64,
    /// The most sprite instances one frame extracted.
    pub max_sprite_instances: u64,
}

impl CounterGuard {
    /// Steps `harness` through `warmup` frames, then counts a window of
    /// `frames` more. Every step counts the sprite channel and copies no
    /// draw, so the harness adds no allocation of its own;
    /// [`Harness::draw`] stays `None` for these frames.
    ///
    /// # Panics
    ///
    /// When the run ends before the window does.
    pub fn measure(harness: &mut Harness, warmup: u32, frames: u32) -> Self {
        // The log console drains the process's one logger, which every
        // thread's app in a test binary feeds: its window would count other
        // tests' records as they happen to land. It counts without the feed.
        let feed = harness
            .world_mut()
            .get_resource_mut::<LogConsole>()
            .and_then(LogConsole::take_feed);
        harness.count_only = true;
        assert_eq!(
            harness.step(warmup),
            warmup,
            "CounterGuard::measure: the run ended in its warm-up"
        );
        let world = harness.world().counters();
        let allocated = allocations();
        let (mut batches, mut instances) = (0, 0);
        for _ in 0..frames {
            assert_eq!(
                harness.step(1),
                1,
                "CounterGuard::measure: the run ended in its window"
            );
            batches = harness.sprite_counts.0.max(batches);
            instances = harness.sprite_counts.1.max(instances);
        }
        let allocated_after = allocations();
        let world_after = harness.world().counters();
        harness.count_only = false;
        if let Some(console) = harness.world_mut().get_resource_mut::<LogConsole>() {
            console.restore_feed(feed);
        }
        Self {
            allocations: allocated_after.count - allocated.count,
            allocated_bytes: allocated_after.bytes - allocated.bytes,
            archetype_moves: world_after.archetype_moves - world.archetype_moves,
            spawned: world_after.spawned - world.spawned,
            despawned: world_after.despawned - world.despawned,
            events_sent: world_after.events_sent - world.events_sent,
            max_sprite_batches: batches,
            max_sprite_instances: instances,
        }
    }

    /// Compares the reading with its pin file at `pin`, one file per guard,
    /// and fails naming each counter that differs, its pinned and its new
    /// value. `TUNGSTEN_GUARDS_WRITE=1` rewrites the pin instead, through a
    /// temporary file and a rename. Pins are Linux's: elsewhere the reading
    /// is printed and the check passes, since platform maths can take
    /// another float path.
    ///
    /// # Panics
    ///
    /// When the reading differs from the pin, the pin is missing or
    /// unreadable, or a write fails.
    #[track_caller]
    pub fn check(&self, pin: impl AsRef<Path>) {
        let pin = pin.as_ref();
        if !cfg!(target_os = "linux") {
            println!(
                "counter guard {}: {self:?}, not compared off Linux",
                pin.display()
            );
            return;
        }
        let result = if std::env::var_os(WRITE_VAR).is_some_and(|value| value == "1") {
            self.write(pin)
        } else {
            self.compare(pin)
        };
        if let Err(message) = result {
            panic!("{message}");
        }
    }

    fn fields(&self) -> [(&'static str, u64); 8] {
        [
            ("allocations", self.allocations),
            ("allocated_bytes", self.allocated_bytes),
            ("archetype_moves", self.archetype_moves),
            ("spawned", self.spawned),
            ("despawned", self.despawned),
            ("events_sent", self.events_sent),
            ("max_sprite_batches", self.max_sprite_batches),
            ("max_sprite_instances", self.max_sprite_instances),
        ]
    }

    fn compare(&self, pin: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(pin).map_err(|err| {
            format!(
                "counter guard {}: no pin to compare ({err}); write it with {WRITE_VAR}=1",
                pin.display()
            )
        })?;
        let pinned: BTreeMap<String, u64> = serde_json::from_str(&text)
            .map_err(|err| format!("counter guard {}: bad pin: {err}", pin.display()))?;
        let fields = self.fields();
        let mut differ = String::new();
        for (name, value) in fields {
            match pinned.get(name) {
                Some(&old) if old == value => {}
                Some(&old) => _ = writeln!(differ, "  {name}: {old} -> {value}"),
                None => _ = writeln!(differ, "  {name}: absent -> {value}"),
            }
        }
        for (name, old) in &pinned {
            if !fields.iter().any(|&(field, _)| field == name) {
                _ = writeln!(differ, "  {name}: {old} -> absent");
            }
        }
        if differ.is_empty() {
            return Ok(());
        }
        Err(format!(
            "counter guard {}: the reading differs from its pin (pinned -> now)\n{differ}\
             rerun with {WRITE_VAR}=1 to rewrite the pin once the change is meant; \
             a rise in a cost counter needs a fix or its cause in the plan",
            pin.display()
        ))
    }

    fn write(&self, pin: &Path) -> Result<(), String> {
        let fields: BTreeMap<&str, u64> = self.fields().into_iter().collect();
        let mut text = serde_json::to_string_pretty(&fields).expect("a map of numbers");
        text.push('\n');
        let mut temporary = pin.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        std::fs::write(&temporary, text)
            .and_then(|()| std::fs::rename(&temporary, pin))
            .map_err(|err| format!("counter guard {}: write failed: {err}", pin.display()))
    }
}

/// A frame's sprite batches and instances, as the renderer would get them.
pub(super) fn sprite_counts(extract: &FrameExtract) -> (u64, u64) {
    let instances = extract
        .sprites
        .iter()
        .map(|batch| batch.instances.len() as u64)
        .sum();
    (extract.sprites.len() as u64, instances)
}

#[cfg(test)]
#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use glam::Vec2;
    use tungsten_core::assets::{FilterMode, UvRect};
    use tungsten_core::{AssetRegistry, Config, EventQueue, Sprite, Transform, Visibility};

    use super::*;
    use crate::App;
    use crate::app::FrameClock;

    fn reading(allocations: u64) -> CounterGuard {
        CounterGuard {
            allocations,
            archetype_moves: 3,
            ..CounterGuard::default()
        }
    }

    /// A fresh folder for one test's pins.
    fn pin_dir(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tungsten-guard-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[derive(Default)]
    struct Ping;

    /// An app whose frames spawn and despawn an entity, send an event and
    /// allocate a buffer.
    fn busy_app() -> App {
        let mut app = App::new(Config::default()).unwrap();
        app.register_event::<Ping>();
        app.add_system(|world| {
            let entity = world.spawn_with((Transform::default(),));
            world.despawn(entity);
            world.resource_mut::<EventQueue<Ping>>().send(Ping);
            let buffer: Vec<u64> = (0..64).collect();
            std::hint::black_box(buffer);
        });
        app
    }

    #[test]
    fn counter_guard_matching_pin_passes() {
        let pin = pin_dir("match").join("guard.json");
        reading(10).write(&pin).unwrap();
        assert_eq!(reading(10).compare(&pin), Ok(()));
    }

    #[test]
    fn counter_guard_mismatch_names_the_counter_both_values_and_the_write_variable() {
        let pin = pin_dir("mismatch").join("guard.json");
        reading(10).write(&pin).unwrap();
        let message = reading(12).compare(&pin).unwrap_err();
        assert!(message.contains("allocations: 10 -> 12"), "{message}");
        assert!(!message.contains("archetype_moves"), "{message}");
        assert!(message.contains("TUNGSTEN_GUARDS_WRITE=1"), "{message}");

        let missing = reading(10)
            .compare(&pin.with_file_name("none.json"))
            .unwrap_err();
        assert!(missing.contains("TUNGSTEN_GUARDS_WRITE=1"), "{missing}");
    }

    #[test]
    fn counter_guard_write_replaces_the_pin_through_a_rename() {
        let dir = pin_dir("write");
        let pin = dir.join("guard.json");
        reading(10).write(&pin).unwrap();
        reading(14).write(&pin).unwrap();
        assert_eq!(reading(14).compare(&pin), Ok(()));
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["guard.json"], "no temporary file is left");
        let text = std::fs::read_to_string(&pin).unwrap();
        assert!(text.contains("\"allocations\": 14"), "{text}");
    }

    #[test]
    fn counter_guard_windows_of_one_app_read_equal_counts_with_no_harness_allocation() {
        let mut harness = Harness::new(busy_app());
        let first = CounterGuard::measure(&mut harness, 10, 30);
        let second = CounterGuard::measure(&mut harness, 0, 30);
        assert_eq!(first, second);
        assert_eq!(first.spawned, 30);
        assert_eq!(first.despawned, 30);
        assert_eq!(first.archetype_moves, 30);
        assert_eq!(first.events_sent, 30);
        assert!(first.allocations >= 30, "{first:?}");
        assert!(harness.draw().is_none(), "a counted frame copies no draw");

        // A counted step allocates what the bare frame body does.
        harness.count_only = true;
        let before = allocations();
        harness.step(1);
        let stepped = allocations().count - before.count;
        harness.app.audio_capture = Some(Vec::new());
        let before = allocations();
        harness
            .app
            .run_frame(Instant::now(), FrameClock::Pinned(harness.dt), |_| {});
        let bare = allocations().count - before.count;
        assert_eq!(stepped, bare);
    }

    #[test]
    fn counter_guard_counts_sprites_on_handles_the_harness_allocates() {
        let mut harness = Harness::new(App::new(Config::default()).unwrap());
        let page = harness.allocate_texture_handle();
        let other = harness.allocate_texture_handle();
        assert_ne!(page, other);
        let registry = harness.world_mut().resource_mut::<AssetRegistry>();
        let mut register = |id: &str, handle| {
            registry.register_sprite(
                id.into(),
                FilterMode::Nearest,
                8,
                8,
                PathBuf::from(format!("__generated__/{id}.png")),
                handle,
                UvRect::FULL,
                None,
                None,
                None,
            )
        };
        let sprites = [
            register("a", page),
            register("b", page),
            register("c", other),
        ];
        for (i, &sprite) in sprites[..2].iter().enumerate() {
            harness.world_mut().spawn_with((
                Transform::from_position(Vec2::new(i as f32 * 10.0, 0.0)),
                Sprite::new(sprite),
                Visibility::default(),
            ));
        }
        let shared = CounterGuard::measure(&mut harness, 1, 2);
        assert_eq!(
            shared.max_sprite_instances, 2,
            "drawn by the default extract"
        );
        assert_eq!(shared.max_sprite_batches, 1, "one handle, one batch");

        harness.world_mut().spawn_with((
            Transform::default(),
            Sprite::new(sprites[2]),
            Visibility::default(),
        ));
        let split = CounterGuard::measure(&mut harness, 1, 2);
        assert_eq!(split.max_sprite_instances, 3);
        assert_eq!(split.max_sprite_batches, 2);
    }
}
