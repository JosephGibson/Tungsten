use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A single frame in an animation sequence.
#[derive(Debug, Clone, Deserialize)]
pub struct AnimationFrame {
    pub sprite: String,
    pub duration_ms: u32,
}

/// Animation data loaded from JSON (per D-010).
#[derive(Debug, Clone, Deserialize)]
pub struct AnimationData {
    pub looping: bool,
    pub frames: Vec<AnimationFrame>,
}

impl AnimationData {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let contents = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read animation '{}': {}",
                path.as_ref().display(),
                e
            )
        })?;
        let data: AnimationData = serde_json::from_str(&contents).map_err(|e| {
            anyhow::anyhow!("Invalid animation '{}': {}", path.as_ref().display(), e)
        })?;
        Ok(data)
    }

    pub fn total_duration_ms(&self) -> u32 {
        self.frames.iter().map(|f| f.duration_ms).sum()
    }
}

/// Registry of loaded animation data, keyed by animation ID.
#[derive(Debug, Default)]
pub struct AnimationRegistry {
    animations: HashMap<String, AnimationData>,
}

impl AnimationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: String, data: AnimationData) {
        self.animations.insert(id, data);
    }

    pub fn get(&self, id: &str) -> Option<&AnimationData> {
        self.animations.get(id)
    }
}

/// ECS component holding animation playback state for an entity.
#[derive(Debug, Clone)]
pub struct AnimationState {
    pub animation_id: String,
    pub frame_index: usize,
    pub accumulated_ms: f32,
    pub playing: bool,
    pub finished: bool,
}

impl AnimationState {
    pub fn new(animation_id: impl Into<String>) -> Self {
        Self {
            animation_id: animation_id.into(),
            frame_index: 0,
            accumulated_ms: 0.0,
            playing: true,
            finished: false,
        }
    }

    /// Get the current sprite ID for this animation state.
    pub fn current_sprite<'a>(&self, registry: &'a AnimationRegistry) -> Option<&'a str> {
        let anim = registry.get(&self.animation_id)?;
        let frame = anim.frames.get(self.frame_index)?;
        Some(&frame.sprite)
    }

    /// Advance the animation by `dt_ms` milliseconds. Returns the new current
    /// sprite ID if the frame changed, or None if it didn't.
    pub fn advance(&mut self, dt_ms: f32, registry: &AnimationRegistry) -> Option<String> {
        if !self.playing || self.finished {
            return None;
        }

        let anim = registry.get(&self.animation_id)?;
        if anim.frames.is_empty() {
            return None;
        }

        self.accumulated_ms += dt_ms;
        let old_frame = self.frame_index;

        loop {
            let current_frame = &anim.frames[self.frame_index];
            if self.accumulated_ms < current_frame.duration_ms as f32 {
                break;
            }

            self.accumulated_ms -= current_frame.duration_ms as f32;
            self.frame_index += 1;

            if self.frame_index >= anim.frames.len() {
                if anim.looping {
                    self.frame_index = 0;
                } else {
                    self.frame_index = anim.frames.len() - 1;
                    self.finished = true;
                    break;
                }
            }
        }

        if self.frame_index != old_frame {
            Some(anim.frames[self.frame_index].sprite.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_anim() -> AnimationData {
        AnimationData {
            looping: true,
            frames: vec![
                AnimationFrame {
                    sprite: "walk_0".into(),
                    duration_ms: 100,
                },
                AnimationFrame {
                    sprite: "walk_1".into(),
                    duration_ms: 100,
                },
                AnimationFrame {
                    sprite: "walk_2".into(),
                    duration_ms: 100,
                },
                AnimationFrame {
                    sprite: "walk_3".into(),
                    duration_ms: 100,
                },
            ],
        }
    }

    #[test]
    fn animation_advances_frames() {
        let mut registry = AnimationRegistry::new();
        registry.insert("walk".into(), test_anim());

        let mut state = AnimationState::new("walk");
        assert_eq!(state.current_sprite(&registry), Some("walk_0"));

        let new = state.advance(150.0, &registry);
        assert_eq!(new, Some("walk_1".into()));
        assert_eq!(state.frame_index, 1);
    }

    #[test]
    fn no_change_within_frame() {
        let mut registry = AnimationRegistry::new();
        registry.insert("walk".into(), test_anim());

        let mut state = AnimationState::new("walk");
        let new = state.advance(50.0, &registry);
        assert_eq!(new, None);
        assert_eq!(state.frame_index, 0);
    }

    #[test]
    fn looping_animation_wraps() {
        let mut registry = AnimationRegistry::new();
        registry.insert("walk".into(), test_anim());

        let mut state = AnimationState::new("walk");
        state.frame_index = 3;
        state.accumulated_ms = 0.0;

        let new = state.advance(150.0, &registry);
        assert_eq!(new, Some("walk_0".into()));
        assert_eq!(state.frame_index, 0);
        assert!(!state.finished);
    }

    #[test]
    fn non_looping_animation_finishes() {
        let mut registry = AnimationRegistry::new();
        let mut anim = test_anim();
        anim.looping = false;
        registry.insert("once".into(), anim);

        let mut state = AnimationState::new("once");
        // Advance through all frames
        state.advance(100.0, &registry); // -> frame 1
        state.advance(100.0, &registry); // -> frame 2
        state.advance(100.0, &registry); // -> frame 3
        let new = state.advance(100.0, &registry); // should finish
        assert_eq!(state.frame_index, 3);
        assert!(state.finished);
        assert_eq!(new, None); // already finished, no change
    }

    #[test]
    fn skip_multiple_frames() {
        let mut registry = AnimationRegistry::new();
        registry.insert("walk".into(), test_anim());

        let mut state = AnimationState::new("walk");
        let new = state.advance(250.0, &registry);
        assert_eq!(new, Some("walk_2".into()));
        assert_eq!(state.frame_index, 2);
    }
}
