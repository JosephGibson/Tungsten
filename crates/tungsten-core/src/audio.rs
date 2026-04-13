use crate::assets::AudioHandle;

/// A command to the audio thread. Game systems push these to `AudioCommands`
/// each tick; the audio callback thread drains them via mpsc.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Begin playing a sound. Overrides the manifest defaults for looping and volume.
    Play {
        handle: AudioHandle,
        /// Volume scale (0.0–1.0). Multiplied by the sound's manifest volume.
        volume: f32,
        /// Whether the sound loops after reaching its end.
        looping: bool,
    },
    /// Stop all active instances of this sound.
    Stop { handle: AudioHandle },
    /// Silence all currently playing sounds.
    StopAll,
    /// Set the master volume (0.0–1.0). Applied to all sounds.
    SetMasterVolume(f32),
}

/// Resource that game systems write audio commands to each tick.
/// The audio callback thread drains it via an mpsc channel set up by `AudioSystem`.
///
/// Game code should push at most a handful of commands per frame. This is a
/// "fire on event" API, not a "call every tick" API.
pub struct AudioCommands {
    commands: Vec<AudioCommand>,
}

impl AudioCommands {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Play a sound using its manifest defaults for looping and volume.
    pub fn play(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume: 1.0,
            looping: false,
        });
    }

    /// Play a sound that loops using its manifest default volume.
    pub fn play_looping(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume: 1.0,
            looping: true,
        });
    }

    /// Play a sound with explicit volume and loop settings.
    pub fn play_with(&mut self, handle: AudioHandle, volume: f32, looping: bool) {
        self.commands.push(AudioCommand::Play {
            handle,
            volume,
            looping,
        });
    }

    /// Stop all active instances of a sound.
    pub fn stop(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Stop { handle });
    }

    /// Stop all currently playing sounds.
    pub fn stop_all(&mut self) {
        self.commands.push(AudioCommand::StopAll);
    }

    /// Set the global master volume (0.0–1.0).
    pub fn set_master_volume(&mut self, volume: f32) {
        self.commands.push(AudioCommand::SetMasterVolume(volume));
    }

    /// Drain all pending commands. Called by `App` after each tick to forward
    /// commands to the audio thread.
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
mod tests {
    use super::*;

    #[test]
    fn drain_returns_all_commands_and_empties_queue() {
        let mut cmds = AudioCommands::new();
        cmds.play(AudioHandle(0));
        cmds.stop(AudioHandle(1));
        cmds.stop_all();
        let drained = cmds.drain();
        assert_eq!(drained.len(), 3);
        assert!(cmds.drain().is_empty());
    }

    #[test]
    fn drain_on_empty_returns_empty() {
        let mut cmds = AudioCommands::new();
        assert!(cmds.drain().is_empty());
    }

    #[test]
    fn play_command_has_correct_defaults() {
        let mut cmds = AudioCommands::new();
        cmds.play(AudioHandle(5));
        let drained = cmds.drain();
        match &drained[0] {
            AudioCommand::Play {
                handle,
                volume,
                looping,
            } => {
                assert_eq!(*handle, AudioHandle(5));
                assert!((volume - 1.0).abs() < 1e-6);
                assert!(!looping);
            }
            _ => panic!("Expected Play command"),
        }
    }

    #[test]
    fn play_looping_sets_looping_true() {
        let mut cmds = AudioCommands::new();
        cmds.play_looping(AudioHandle(3));
        let drained = cmds.drain();
        match &drained[0] {
            AudioCommand::Play { looping, .. } => assert!(*looping),
            _ => panic!("Expected Play command"),
        }
    }

    #[test]
    fn set_master_volume_queues_correctly() {
        let mut cmds = AudioCommands::new();
        cmds.set_master_volume(0.5);
        let drained = cmds.drain();
        assert!(matches!(drained[0], AudioCommand::SetMasterVolume(v) if (v - 0.5).abs() < 1e-6));
    }

    #[test]
    fn stop_all_queues_correctly() {
        let mut cmds = AudioCommands::new();
        cmds.stop_all();
        let drained = cmds.drain();
        assert!(matches!(drained[0], AudioCommand::StopAll));
    }
}
