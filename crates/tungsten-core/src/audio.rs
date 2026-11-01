//! What a game asks the mixer to play: commands queued on [`AudioCommands`]
//! and sent to the mixer once a frame (`D-034`). Every sound plays on a
//! [`AudioBus`], whose volume is the player's, and every play starts a
//! [`Voice`] that later commands fade, set or stop.

use serde::{Deserialize, Serialize};

use crate::World;
use crate::assets::{AudioHandle, SoundRegistry};

/// The voices the mixer plays at once, and the largest voice limit a sound
/// takes in the manifest (`max_voices`).
pub const MAX_VOICES: u32 = 64;

/// A mixer bus. Every sound plays on one, from its manifest entry's `bus`;
/// a voice plays at its volume × its bus's volume × the master volume. A
/// bus's volume is the player's, which `tungsten::apply_settings` sets from
/// the settings; a game mixes with voice volumes and fades. A match outside
/// this crate needs a wildcard arm, since later buses are additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioBus {
    /// Music: `"music"`.
    Music,
    /// Sound effects, the default: `"effects"`.
    #[default]
    Effects,
}

impl AudioBus {
    /// Every bus, in order.
    pub const ALL: &'static [AudioBus] = &[AudioBus::Music, AudioBus::Effects];

    /// The bus's name in the manifest and the settings file.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Effects => "effects",
        }
    }
}

/// One playing instance of a sound, which a play on [`AudioCommands`]
/// returns and [`stop_voice`](AudioCommands::stop_voice) and
/// [`set_voice_volume`](AudioCommands::set_voice_volume) name.
///
/// An `AudioCommands` numbers its voices from 1 and never reuses one. A
/// command naming a voice that has ended, was dropped or never started does
/// nothing. A replaced `AudioCommands` resource numbers from 1 again, so it
/// can address a voice the old one started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Voice(u64);

/// Where a voice sits between the speakers (redo 7.9): -1 (left) to 1
/// (right), 0 centred, the default, kept in steps of 1/16,384, so an
/// [`AudioCommand`] stays 24 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Pan(i16);

impl Pan {
    /// Steps per unit.
    const STEPS: f32 = 16_384.0;
    /// Both speakers alike.
    pub const CENTRE: Pan = Pan(0);

    /// `pan` clamped to -1..=1, to the nearest step; one that is not finite
    /// is centred.
    #[must_use]
    pub fn new(pan: f32) -> Self {
        if pan.is_finite() {
            Self((pan.clamp(-1.0, 1.0) * Self::STEPS).round() as i16)
        } else {
            Self::CENTRE
        }
    }

    /// The pan, -1 to 1.
    #[must_use]
    pub fn get(self) -> f32 {
        f32::from(self.0) / Self::STEPS
    }
}

/// A command for the mixer. A match outside this crate needs a wildcard arm,
/// since later commands are additions, and code outside it builds one
/// through [`AudioCommands`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AudioCommand {
    /// Start `voice`, playing `handle`.
    #[non_exhaustive]
    Play {
        /// The sound.
        handle: AudioHandle,
        /// The voice this play starts, which later commands name.
        voice: Voice,
        /// Volume scale.
        volume: f32,
        /// Loop at end.
        looping: bool,
        /// Seconds over which the voice rises from 0 to `volume`; 0 starts
        /// it at `volume`.
        fade: f32,
        /// Where it sits between the speakers (redo 7.9).
        pan: Pan,
    },
    /// Stop active instances of sound.
    Stop {
        /// The sound whose voices stop.
        handle: AudioHandle,
    },
    /// Stop all sounds.
    StopAll,
    /// Set master volume.
    SetMasterVolume(f32),
    /// Stop `voice`, ramping it to 0 over `fade` seconds.
    #[non_exhaustive]
    StopVoice {
        /// The voice.
        voice: Voice,
        /// Seconds over which it falls to 0.
        fade: f32,
    },
    /// Move `voice` to `volume` over `fade` seconds.
    #[non_exhaustive]
    SetVoiceVolume {
        /// The voice.
        voice: Voice,
        /// Its new volume scale.
        volume: f32,
        /// Seconds over which it moves there.
        fade: f32,
    },
    /// Set `bus`'s volume, 0 to 1.
    #[non_exhaustive]
    SetBusVolume {
        /// The bus.
        bus: AudioBus,
        /// Its volume.
        volume: f32,
    },
    /// Move `voice`'s pan to `pan` over `fade` seconds (redo 7.9).
    #[non_exhaustive]
    SetVoicePan {
        /// The voice.
        voice: Voice,
        /// Its new pan.
        pan: Pan,
        /// Seconds over which it moves there.
        fade: f32,
    },
    /// Move `bus`'s duck to `gain` over `fade` seconds (redo 7.9): a gain,
    /// 0 to 1 and 1 by default, on top of the player's volume, which stays
    /// as set.
    #[non_exhaustive]
    DuckBus {
        /// The bus.
        bus: AudioBus,
        /// Its duck, 0 to 1.
        gain: f32,
        /// Seconds over which it moves there.
        fade: f32,
    },
}

/// Per-frame audio command queue resource.
///
/// A volume that is not finite or is negative is 0 at every entry point, and
/// so are seconds that are not finite or are negative.
pub struct AudioCommands {
    commands: Vec<AudioCommand>,
    /// The number the next play's voice takes.
    next_voice: u64,
}

impl AudioCommands {
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            next_voice: 1,
        }
    }

    /// Play once; returns the voice it starts.
    pub fn play(&mut self, handle: AudioHandle) -> Voice {
        self.push_play(handle, 1.0, false, 0.0, Pan::CENTRE)
    }

    /// Play looping; returns the voice it starts.
    pub fn play_looping(&mut self, handle: AudioHandle) -> Voice {
        self.push_play(handle, 1.0, true, 0.0, Pan::CENTRE)
    }

    /// Play with explicit volume/loop; returns the voice it starts. A volume
    /// that is not finite or is negative plays at 0.
    pub fn play_with(&mut self, handle: AudioHandle, volume: f32, looping: bool) -> Voice {
        self.push_play(handle, sanitize_volume(volume), looping, 0.0, Pan::CENTRE)
    }

    /// Play at `pan`, -1 (left) to 1 (right), clamped, a pan that is not
    /// finite centred (redo 7.9); returns the voice it starts.
    pub fn play_panned(
        &mut self,
        handle: AudioHandle,
        volume: f32,
        looping: bool,
        pan: f32,
    ) -> Voice {
        self.push_play(handle, sanitize_volume(volume), looping, 0.0, Pan::new(pan))
    }

    /// Play rising from 0 to `volume` over `seconds`, in one command, so the
    /// fade never comes apart from its start; returns the voice it starts.
    /// A crossfade into the boss's music:
    ///
    /// ```
    /// use tungsten_core::assets::AudioHandle;
    /// use tungsten_core::AudioCommands;
    ///
    /// let (battle_theme, boss_theme) = (AudioHandle(0), AudioHandle(1));
    /// let mut audio = AudioCommands::new();
    /// let battle = audio.play_looping(battle_theme);
    /// // The boss arrives: the battle music falls as the boss's rises.
    /// audio.stop_voice(battle, 2.0);
    /// let boss = audio.play_faded(boss_theme, 0.8, true, 2.0);
    /// assert_ne!(battle, boss);
    /// assert_eq!(audio.drain().len(), 3);
    /// ```
    pub fn play_faded(
        &mut self,
        handle: AudioHandle,
        volume: f32,
        looping: bool,
        seconds: f32,
    ) -> Voice {
        self.push_play(
            handle,
            sanitize_volume(volume),
            looping,
            sanitize_seconds(seconds),
            Pan::CENTRE,
        )
    }

    /// Stop active instances of sound.
    pub fn stop(&mut self, handle: AudioHandle) {
        self.commands.push(AudioCommand::Stop { handle });
    }

    /// Stop all sounds.
    pub fn stop_all(&mut self) {
        self.commands.push(AudioCommand::StopAll);
    }

    /// Stop `voice`, ramping it to 0 over `seconds`.
    pub fn stop_voice(&mut self, voice: Voice, seconds: f32) {
        self.commands.push(AudioCommand::StopVoice {
            voice,
            fade: sanitize_seconds(seconds),
        });
    }

    /// Move `voice` to `volume` over `seconds`.
    pub fn set_voice_volume(&mut self, voice: Voice, volume: f32, seconds: f32) {
        self.commands.push(AudioCommand::SetVoiceVolume {
            voice,
            volume: sanitize_volume(volume),
            fade: sanitize_seconds(seconds),
        });
    }

    /// Move `voice`'s pan to `pan` over `seconds` (redo 7.9), clamped as
    /// [`play_panned`](Self::play_panned)'s.
    pub fn set_voice_pan(&mut self, voice: Voice, pan: f32, seconds: f32) {
        self.commands.push(AudioCommand::SetVoicePan {
            voice,
            pan: Pan::new(pan),
            fade: sanitize_seconds(seconds),
        });
    }

    /// Ducks `bus` to `gain`, 0 to 1, over `seconds` (redo 7.9): the bus
    /// plays at the player's volume × its duck, the volume staying as set;
    /// a duck of 1 lifts it.
    pub fn duck_bus(&mut self, bus: AudioBus, gain: f32, seconds: f32) {
        self.commands.push(AudioCommand::DuckBus {
            bus,
            gain: sanitize_volume(gain).min(1.0),
            fade: sanitize_seconds(seconds),
        });
    }

    /// Set master volume; one that is not finite or is negative is 0.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.commands
            .push(AudioCommand::SetMasterVolume(sanitize_volume(volume)));
    }

    /// Set `bus`'s volume, capped at 1. A bus's volume is the player's:
    /// `tungsten::apply_settings` sets every bus from the settings.
    pub fn set_bus_volume(&mut self, bus: AudioBus, volume: f32) {
        self.commands.push(AudioCommand::SetBusVolume {
            bus,
            volume: sanitize_volume(volume).min(1.0),
        });
    }

    /// Drain pending commands after tick.
    pub fn drain(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.commands)
    }

    fn push_play(
        &mut self,
        handle: AudioHandle,
        volume: f32,
        looping: bool,
        fade: f32,
        pan: Pan,
    ) -> Voice {
        let voice = Voice(self.next_voice);
        self.next_voice += 1;
        self.commands.push(AudioCommand::Play {
            handle,
            voice,
            volume,
            looping,
            fade,
            pan,
        });
        voice
    }
}

/// Plays the manifest's sound `id` at its volume, looping as its entry
/// says, faded in over `fade` seconds, centred (redo 7.9); its voice, or
/// `None` when no sound has that ID or the World holds no
/// [`SoundRegistry`] or [`AudioCommands`].
pub fn play(world: &mut World, id: &str, fade: f32) -> Option<Voice> {
    let sounds = world.get_resource::<SoundRegistry>()?;
    let handle = sounds.get_by_id(id)?;
    let (volume, looping) = (sounds.get_volume(handle), sounds.get_looping(handle));
    let audio = world.get_resource_mut::<AudioCommands>()?;
    Some(audio.play_faded(handle, volume, looping, fade))
}

/// A volume the mixer can multiply by: one that is not finite or is
/// negative becomes 0, so no NaN or inverted gain reaches the samples.
fn sanitize_volume(volume: f32) -> f32 {
    if volume.is_finite() && volume > 0.0 {
        volume
    } else {
        0.0
    }
}

/// A fade the mixer can count frames by: seconds that are not finite or are
/// negative become 0.
fn sanitize_seconds(seconds: f32) -> f32 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
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
