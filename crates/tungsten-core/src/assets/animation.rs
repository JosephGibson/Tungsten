use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Animation frame.
#[derive(Debug, Clone, Deserialize)]
pub struct AnimationFrame {
    pub sprite: String,
    pub duration_ms: u32,
}

/// D-010 animation data.
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

    #[must_use]
    pub fn total_duration_ms(&self) -> u32 {
        self.frames.iter().map(|f| f.duration_ms).sum()
    }
}

/// Animation registry resource.
#[derive(Debug, Default, Clone)]
pub struct AnimationRegistry {
    animations: HashMap<String, AnimationData>,
    path_to_id: HashMap<PathBuf, String>,
}

impl AnimationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: String, data: AnimationData) {
        self.animations.insert(id, data);
    }

    /// Insert with source-path reverse lookup.
    pub fn insert_with_path(&mut self, id: String, data: AnimationData, path: PathBuf) {
        self.path_to_id.insert(path, id.clone());
        self.animations.insert(id, data);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AnimationData> {
        self.animations.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &AnimationData)> {
        self.animations.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Animation ID for source path.
    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.animations.keys().map(String::as_str)
    }
}

/// Animation playback component.
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

    /// Current sprite ID.
    #[must_use]
    pub fn current_sprite<'a>(&self, registry: &'a AnimationRegistry) -> Option<&'a str> {
        let anim = registry.get(&self.animation_id)?;
        let frame = anim.frames.get(self.frame_index)?;
        Some(&frame.sprite)
    }

    /// Advance by milliseconds; returns sprite ID on frame change.
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

        // Bound zero-duration frame loops.
        let max_steps = anim.frames.len() * 2;
        for _ in 0..max_steps {
            let current_frame = &anim.frames[self.frame_index];
            let dur = (current_frame.duration_ms as f32).max(1.0);
            if self.accumulated_ms < dur {
                break;
            }

            self.accumulated_ms -= dur;
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
#[path = "../tests/assets/animation.rs"]
mod tests;
