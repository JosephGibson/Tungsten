//! Sound on an event: a manifest sound played once per event.

use std::collections::{HashMap, HashSet};
use std::fmt;

use tungsten_core::{
    AudioCommands, AudioHandle, EventCursor, EventQueue, Plugin, Schedule, SoundRegistry, Time,
    World, system,
};

use crate::foundation::phases::{Phase, ensure_phases};

/// Plays a manifest sound for each event `E`, once per event: a hit, a
/// kill, a pickup. The plugin registers `EventQueue<E>` (again is a no-op)
/// and its system in the present phase (`PostUpdate`) under the name the
/// game passes, as any system takes one; two sound plugins need two names.
///
/// The system reads its queue through an [`EventCursor`], so it plays each
/// event once, whatever its place among the senders: an event sent before
/// it in the frame plays that frame, one sent after it plays the next, and
/// none plays twice; a queue replaced between runs plays its own events
/// from the first. It plays at the manifest's
/// volume, never looping, through `AudioCommands`. Audio runs on real time,
/// so events sent while the game clock is paused play. With
/// [`with_min_interval`](Self::with_min_interval), a sound ID plays at most
/// once per interval of real time, and the events between are read and
/// dropped. An unknown sound ID logs one warning per ID and plays nothing,
/// and a world without a `SoundRegistry` or `AudioCommands` plays nothing.
///
/// # Examples
///
/// ```
/// use tungsten_core::{
///     AudioCommand, AudioCommands, EventQueue, Plugin, Schedule, SoundData, SoundRegistry,
///     Stage, World,
/// };
/// use tungsten_kit::SoundOnEvent;
///
/// struct Pickup;
///
/// let mut world = World::new();
/// let mut sounds = SoundRegistry::new();
/// let chime = SoundData { samples: vec![0.0; 4], sample_rate: 44_100, channels: 1 };
/// let handle = sounds.register("chime".to_string(), chime, 0.5, false);
/// world.insert_resource(sounds);
/// world.insert_resource(AudioCommands::new());
/// let mut schedule = Schedule::new();
/// SoundOnEvent::<Pickup>::new("pickup_sound", "chime").build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// world.resource_mut::<EventQueue<Pickup>>().send(Pickup);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// let played = world.resource_mut::<AudioCommands>().drain();
/// assert!(matches!(
///     played[..],
///     [AudioCommand::Play { handle: h, volume: 0.5, looping: false }] if h == handle
/// ));
/// ```
pub struct SoundOnEvent<E> {
    name: &'static str,
    pick: Pick<E>,
    min_interval: f32,
}

/// Which sound an event plays.
enum Pick<E> {
    Every(&'static str),
    Select(fn(&E) -> Option<&str>),
}

impl<E> Clone for Pick<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E> Copy for Pick<E> {}

impl<E: 'static> SoundOnEvent<E> {
    /// Every event plays `sound`, a manifest sound ID; the system runs as
    /// `name`.
    #[must_use]
    pub fn new(name: &'static str, sound: &'static str) -> Self {
        Self {
            name,
            pick: Pick::Every(sound),
            min_interval: 0.0,
        }
    }

    /// Each event plays the sound `pick` names for it, and `None` plays
    /// nothing; the system runs as `name`.
    #[must_use]
    pub fn select(name: &'static str, pick: fn(&E) -> Option<&str>) -> Self {
        Self {
            name,
            pick: Pick::Select(pick),
            min_interval: 0.0,
        }
    }

    /// The plugin, playing each sound ID at most once per `seconds` of real
    /// time, as `Time::real_elapsed` counts it; 0, the default, plays every
    /// event. A burst of hits then plays one sound.
    ///
    /// ```
    /// use tungsten_core::{
    ///     AudioCommands, EventQueue, Plugin, Schedule, SoundData, SoundRegistry, Stage, Time,
    ///     World,
    /// };
    /// use tungsten_kit::SoundOnEvent;
    ///
    /// struct Hit;
    ///
    /// let mut world = World::new();
    /// world.insert_resource(Time::new());
    /// let mut sounds = SoundRegistry::new();
    /// let thud = SoundData { samples: vec![0.0; 4], sample_rate: 44_100, channels: 1 };
    /// sounds.register("thud".to_string(), thud, 1.0, false);
    /// world.insert_resource(sounds);
    /// world.insert_resource(AudioCommands::new());
    /// let mut schedule = Schedule::new();
    /// SoundOnEvent::<Hit>::new("hit_sound", "thud")
    ///     .with_min_interval(0.1)
    ///     .build(&mut schedule, &mut world);
    /// schedule.resolve().unwrap();
    ///
    /// // Three hits in one frame play once.
    /// for _ in 0..3 {
    ///     world.resource_mut::<EventQueue<Hit>>().send(Hit);
    /// }
    /// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    /// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    /// assert_eq!(world.resource_mut::<AudioCommands>().drain().len(), 1);
    /// ```
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_min_interval(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "SoundOnEvent::with_min_interval: seconds must be finite and >= 0 (got {seconds})"
        );
        self.min_interval = seconds;
        self
    }
}

impl<E> Clone for SoundOnEvent<E> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            pick: self.pick,
            min_interval: self.min_interval,
        }
    }
}

impl<E> fmt::Debug for SoundOnEvent<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = f.debug_struct("SoundOnEvent");
        out.field("name", &self.name);
        match self.pick {
            Pick::Every(sound) => out.field("sound", &sound),
            Pick::Select(_) => out.field("sound", &"selected per event"),
        };
        out.field("min_interval", &self.min_interval);
        out.finish()
    }
}

impl<E: 'static> Plugin for SoundOnEvent<E> {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<E>();
        let mut reader = Reader {
            pick: self.pick,
            min_interval: self.min_interval,
            cursor: EventCursor::new(),
            warned: HashSet::new(),
            last: HashMap::new(),
            play: Vec::new(),
        };
        ensure_phases(schedule);
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(self.name, move |world: &mut World| {
                reader.run(world);
            })),
        );
    }
}

/// Seconds an interval's end may be early by and still pass, against the
/// sum's rounding.
const INTERVAL_SLACK: f64 = 1e-6;

/// The system's state between frames.
struct Reader<E> {
    pick: Pick<E>,
    min_interval: f32,
    /// The system's place in the queue: what it has played.
    cursor: EventCursor<E>,
    /// The unknown sound IDs already warned about.
    warned: HashSet<String>,
    /// When each sound ID last played, in real seconds, while an interval
    /// applies.
    last: HashMap<String, f64>,
    /// This run's sounds and their volumes, kept so a frame allocates
    /// nothing once warm.
    play: Vec<(AudioHandle, f32)>,
}

impl<E: 'static> Reader<E> {
    fn run(&mut self, world: &mut World) {
        let Some(queue) = world.get_resource::<EventQueue<E>>() else {
            return;
        };
        // The read moves the cursor past every event at once, so with no
        // registry or commands to play them nothing replays later.
        let events = self.cursor.read(queue);
        if events.len() == 0 {
            return;
        }
        let Some(sounds) = world.get_resource::<SoundRegistry>() else {
            return;
        };
        let now = world.get_resource::<Time>().map(Time::real_elapsed);
        let interval = f64::from(self.min_interval);
        self.play.clear();
        for event in events {
            let id = match self.pick {
                Pick::Every(sound) => sound,
                Pick::Select(pick) => match pick(event) {
                    Some(id) => id,
                    None => continue,
                },
            };
            match sounds.get_by_id(id) {
                Some(handle) => {
                    if interval > 0.0
                        && let Some(now) = now
                    {
                        if let Some(&last) = self.last.get(id)
                            && now - last + INTERVAL_SLACK < interval
                        {
                            continue;
                        }
                        self.last.insert(id.to_owned(), now);
                    }
                    self.play.push((handle, sounds.get_volume(handle)));
                }
                None => {
                    if !self.warned.contains(id) {
                        log::warn!(
                            "SoundOnEvent: no sound '{id}' in the manifest; it plays nothing"
                        );
                        self.warned.insert(id.to_owned());
                    }
                }
            }
        }
        let Some(commands) = world.get_resource_mut::<AudioCommands>() else {
            return;
        };
        for &(handle, volume) in &self.play {
            commands.play_with(handle, volume, false);
        }
    }
}

#[cfg(test)]
#[path = "../tests/sound.rs"]
mod tests;
