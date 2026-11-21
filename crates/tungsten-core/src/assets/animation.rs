use serde::Deserialize;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::registry::{AssetRegistry, SpriteAssetId};

/// Animation frame. The file names its sprite; [`AnimationData::load`] interns it.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub sprite: SpriteAssetId,
    pub duration_ms: u32,
    /// A manifest sound the kit plays as the clip enters this frame (redo
    /// 7.9), its ID checked against the sounds at load.
    pub sound: Option<String>,
}

/// D-010 animation data.
#[derive(Debug, Clone)]
pub struct AnimationData {
    pub looping: bool,
    pub frames: Vec<AnimationFrame>,
}

/// On-disk animation: frames name their sprites.
#[derive(Deserialize)]
struct AnimationFile {
    looping: bool,
    frames: Vec<AnimationFileFrame>,
}

#[derive(Deserialize)]
struct AnimationFileFrame {
    sprite: String,
    duration_ms: u32,
    #[serde(default)]
    sound: Option<String>,
}

/// Error loading an animation file.
#[derive(Debug, Error)]
pub enum AnimationError {
    #[error("Failed to read animation '{path}': {error}")]
    Read { path: String, error: std::io::Error },
    #[error("Invalid animation '{path}': {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
}

impl AnimationData {
    /// Loads an animation file, interning each frame's sprite name in `sprites`.
    pub fn load(
        path: impl AsRef<Path>,
        sprites: &mut AssetRegistry,
    ) -> Result<Self, AnimationError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|error| AnimationError::Read {
            path: path.display().to_string(),
            error,
        })?;
        let file: AnimationFile =
            serde_json::from_str(&contents).map_err(|error| AnimationError::Parse {
                path: path.display().to_string(),
                error,
            })?;
        Ok(Self {
            looping: file.looping,
            frames: file
                .frames
                .into_iter()
                .map(|frame| AnimationFrame {
                    sprite: sprites.intern_sprite(&frame.sprite),
                    duration_ms: frame.duration_ms,
                    sound: frame.sound,
                })
                .collect(),
        })
    }

    /// The frames' durations summed, saturating at `u32::MAX`.
    #[must_use]
    pub fn total_duration_ms(&self) -> u32 {
        self.frames.iter().fold(0, |total: u32, frame| {
            total.saturating_add(frame.duration_ms)
        })
    }
}

/// Animation registry resource: the clips in the order they registered,
/// each at a slot an [`AnimationState`] keeps, so a state on a registered
/// clip finds it by one compare of its ID.
#[derive(Debug, Default, Clone)]
pub struct AnimationRegistry {
    clips: Vec<Clip>,
    slots: FixedMap<String, u32>,
    /// Each source path's clip, by slot.
    paths: FixedMap<PathBuf, u32>,
}

/// A map under a hasher that takes no room in it, so the registry, which
/// five systems box again each frame, stays under its old 96 bytes.
type FixedMap<K, V> = HashMap<K, V, BuildHasherDefault<DefaultHasher>>;

/// One registered clip: its ID and its data.
#[derive(Debug, Clone)]
struct Clip {
    id: String,
    data: AnimationData,
}

/// [`AnimationState`]'s slot before its first lookup.
const NO_SLOT: u32 = u32::MAX;

/// Whether two IDs are equal, a word at a time with no call: the lengths,
/// then each eight bytes and the last eight, which overlap the others in an
/// ID whose length is no multiple of eight; an ID under eight bytes goes a
/// byte at a time.
#[inline]
fn same_id(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let word = |bytes: &[u8]| u64::from_ne_bytes(bytes.try_into().expect("eight bytes"));
    if a.len() < 8 {
        return a.iter().zip(b).all(|(x, y)| x == y);
    }
    let last = a.len() - 8;
    let mut at = 0;
    while at < last {
        if word(&a[at..at + 8]) != word(&b[at..at + 8]) {
            return false;
        }
        at += 8;
    }
    word(&a[last..]) == word(&b[last..])
}

impl AnimationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `data` under `id`: a new ID takes the next slot, a held
    /// one keeps its slot with the new data.
    pub fn insert(&mut self, id: String, data: AnimationData) {
        self.put(id, data);
    }

    /// Insert with source-path reverse lookup.
    pub fn insert_with_path(&mut self, id: String, data: AnimationData, path: PathBuf) {
        let slot = self.put(id, data);
        self.paths.insert(path, slot);
    }

    /// [`insert`](Self::insert), and the slot the clip holds.
    fn put(&mut self, id: String, data: AnimationData) -> u32 {
        if let Some(&slot) = self.slots.get(&id) {
            self.clips[slot as usize].data = data;
            return slot;
        }
        let slot = u32::try_from(self.clips.len()).expect("fewer than 2^32 clips");
        self.slots.insert(id.clone(), slot);
        self.clips.push(Clip { id, data });
        slot
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AnimationData> {
        let &slot = self.slots.get(id)?;
        Some(&self.clips[slot as usize].data)
    }

    /// The clip `id` names and its slot: the one at `slot` when that slot
    /// holds `id`, one compare, else the lookup by name.
    #[inline]
    fn find(&self, slot: u32, id: &str) -> Option<(u32, &AnimationData)> {
        match self.clips.get(slot as usize) {
            Some(clip) if same_id(&clip.id, id) => Some((slot, &clip.data)),
            _ => {
                let &slot = self.slots.get(id)?;
                Some((slot, &self.clips[slot as usize].data))
            }
        }
    }

    /// Each clip's ID and data, in the order the IDs first registered.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AnimationData)> {
        self.clips.iter().map(|clip| (clip.id.as_str(), &clip.data))
    }

    /// Animation ID for source path.
    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        let &slot = self.paths.get(path)?;
        Some(self.clips[slot as usize].id.as_str())
    }

    /// The clips' IDs, in the order they first registered.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.clips.iter().map(|clip| clip.id.as_str())
    }
}

/// Animation playback component.
#[derive(Debug, Clone)]
pub struct AnimationState {
    pub animation_id: String,
    /// The frame shown; a clip holds fewer than 2^32.
    pub frame_index: u32,
    /// Its clip's slot in the registry that last advanced it, checked
    /// against [`animation_id`](Self::animation_id) at each use.
    slot: u32,
    pub accumulated_ms: f32,
    pub playing: bool,
    pub finished: bool,
    /// The [`speed`](Self::speed) in thousandths, low 15 bits, and
    /// [`despawn_at_end`](Self::despawn_at_end), the top bit (redo 7.9):
    /// two bytes in the padding, so the component stays 40 bytes, which
    /// example 01's allocation guard pins.
    rate: u16,
}

/// [`AnimationState`]'s speed bits and its despawn bit.
const SPEED_BITS: u16 = 0x7FFF;
const DESPAWN_BIT: u16 = 0x8000;
/// Speed steps per unit.
const SPEED_STEPS: f32 = 1000.0;

/// What one [`AnimationState::advance_entering`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Advanced {
    /// The new frame's sprite, on a frame change.
    pub sprite: Option<SpriteAssetId>,
    /// The last frame the advance entered that names a sound.
    pub sound_frame: Option<usize>,
}

impl AnimationState {
    pub fn new(animation_id: impl Into<String>) -> Self {
        Self {
            animation_id: animation_id.into(),
            frame_index: 0,
            slot: NO_SLOT,
            accumulated_ms: 0.0,
            playing: true,
            finished: false,
            rate: SPEED_STEPS as u16,
        }
    }

    /// The playback rate (redo 7.9): 1 plays the frames at their durations,
    /// 0.5 at half speed, 0 holds the frame; 1 unless set.
    #[must_use]
    pub fn speed(&self) -> f32 {
        f32::from(self.rate & SPEED_BITS) / SPEED_STEPS
    }

    /// Sets the playback rate, to the nearest thousandth up to 32.767; one
    /// that is not finite or is negative reads 1.
    pub fn set_speed(&mut self, speed: f32) {
        let speed = if speed.is_finite() && speed >= 0.0 {
            speed.min(f32::from(SPEED_BITS) / SPEED_STEPS)
        } else {
            1.0
        };
        self.rate = (self.rate & DESPAWN_BIT) | (speed * SPEED_STEPS).round() as u16;
    }

    /// The state, at `speed` ([`set_speed`](Self::set_speed)).
    #[must_use]
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.set_speed(speed);
        self
    }

    /// Whether the kit ends its entity once a one-shot clip finishes (redo
    /// 7.9); a looping clip never finishes. Off unless set.
    #[must_use]
    pub fn despawn_at_end(&self) -> bool {
        self.rate & DESPAWN_BIT != 0
    }

    /// Sets whether a finished one-shot ends its entity.
    pub fn set_despawn_at_end(&mut self, despawn: bool) {
        self.rate = (self.rate & SPEED_BITS) | if despawn { DESPAWN_BIT } else { 0 };
    }

    /// The state, ending its entity at a one-shot's end.
    #[must_use]
    pub fn with_despawn_at_end(mut self, despawn: bool) -> Self {
        self.set_despawn_at_end(despawn);
        self
    }

    /// Current frame's sprite ID.
    #[must_use]
    #[inline]
    pub fn current_sprite(&self, registry: &AnimationRegistry) -> Option<SpriteAssetId> {
        let (_, anim) = registry.find(self.slot, &self.animation_id)?;
        let frame = anim.frames.get(self.frame_index as usize)?;
        Some(frame.sprite)
    }

    /// Advance by milliseconds at its [`speed`](Self::speed); returns the new
    /// frame's sprite ID on a frame change.
    #[inline]
    pub fn advance(&mut self, dt_ms: f32, registry: &AnimationRegistry) -> Option<SpriteAssetId> {
        self.advance_entering(dt_ms, registry).sprite
    }

    /// [`advance`](Self::advance), and the last frame it entered that names
    /// a sound (redo 7.9): a frame is entered when playback moves into it,
    /// a loop's wrap into the first frame included, so a clip's first frame
    /// sounds on each wrap, never at the clip's start.
    #[inline]
    pub fn advance_entering(&mut self, dt_ms: f32, registry: &AnimationRegistry) -> Advanced {
        if !self.playing || self.finished {
            return Advanced::default();
        }
        match registry.find(self.slot, &self.animation_id) {
            Some((slot, clip)) => {
                self.slot = slot;
                self.advance_in(dt_ms, clip)
            }
            None => Advanced::default(),
        }
    }

    /// [`advance_entering`](Self::advance_entering) in `clip`, the state's
    /// animation as its caller found it: a pass over many states on one
    /// clip looks it up once.
    #[inline]
    pub fn advance_in(&mut self, dt_ms: f32, clip: &AnimationData) -> Advanced {
        let anim = clip;
        let mut advanced = Advanced::default();
        if !self.playing || self.finished || anim.frames.is_empty() {
            return advanced;
        }

        self.accumulated_ms += dt_ms * self.speed();
        let old_frame = self.frame_index;
        // A hot reload may replace the clip with fewer frames while playback
        // still points into the old clip. Preserve progress up to its new end.
        let mut frame = (self.frame_index as usize).min(anim.frames.len() - 1);

        // Bound zero-duration frame loops.
        let max_steps = anim.frames.len() * 2;
        for _ in 0..max_steps {
            let current_frame = &anim.frames[frame];
            let dur = (current_frame.duration_ms as f32).max(1.0);
            if self.accumulated_ms < dur {
                break;
            }

            self.accumulated_ms -= dur;
            frame += 1;

            if frame >= anim.frames.len() {
                if anim.looping {
                    frame = 0;
                } else {
                    frame = anim.frames.len() - 1;
                    self.finished = true;
                    break;
                }
            }
            if anim.frames[frame].sound.is_some() {
                advanced.sound_frame = Some(frame);
            }
        }

        // A clip holds fewer than 2^32 frames.
        self.frame_index = frame as u32;
        if self.frame_index != old_frame {
            advanced.sprite = Some(anim.frames[frame].sprite);
        }
        advanced
    }
}

#[cfg(test)]
#[path = "../tests/assets/animation.rs"]
mod tests;
