use crate::assets::AudioHandle;

/// Audio thread command.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Begin playback.
    Play {
        handle: AudioHandle,
        /// Volume scale.
        volume: f32,
        /// Loop at end.
        looping: bool,
    },
    /// Stop active instances of sound.
    Stop { handle: AudioHandle },
    /// Stop all sounds.
    StopAll,
    /// Set master volume.
    SetMasterVolume(f32),
}

/// Per-frame audio command queue resource.
pub struct AudioCommands {
    commands: Vec<AudioCommand>,
}

impl AudioCommands {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Play once.
    pub fn play(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume: 1.0,
            looping: false,
        });
    }

    /// Play looping.
    pub fn play_looping(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume: 1.0,
            looping: true,
        });
    }

    /// Play with explicit volume/loop.
    pub fn play_with(&mut self, handle: AudioHandle, volume: f32, looping: bool) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume,
            looping,
        });
    }

    /// Stop active instances of sound.
    pub fn stop(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Stop { handle });
    }

    /// Stop all sounds.
    pub fn stop_all(&mut self) {
        self.commands.push(AudioCommand::StopAll);
    }

    /// Set master volume.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.commands.push(AudioCommand::SetMasterVolume(volume));
    }

    /// Drain pending commands after tick.
    pub fn drain(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.commands)
    }
}

impl Default for AudioCommands {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/audio.rs"]
mod tests;
