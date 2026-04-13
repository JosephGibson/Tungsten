use std::collections::HashMap;
use std::sync::mpsc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tungsten_core::assets::{AudioHandle, SoundData, SoundRegistry};
use tungsten_core::audio::AudioCommand;

/// Internal state for one playing sound in the mixer.
struct PlayingSound {
    handle: AudioHandle,
    /// Current read position in `SoundData::samples` (interleaved stereo index).
    cursor: usize,
    /// Per-instance volume scale (0.0–1.0).
    volume: f32,
    looping: bool,
    finished: bool,
}

/// The audio subsystem. Owns the `cpal::Stream` (must stay alive) and the
/// sender end of the command channel. Created once during `App::resumed()`.
pub struct AudioSystem {
    /// Kept alive so the cpal stream continues playing.
    _stream: cpal::Stream,
    sender: mpsc::Sender<AudioCommand>,
}

impl AudioSystem {
    /// Initialize the audio subsystem. Opens a cpal output device, clones all
    /// decoded sound data from the registry into the callback closure, and
    /// spawns the stream. The stream runs on cpal's own thread.
    pub fn init(sound_registry: &SoundRegistry) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No audio output device found"))?;

        let config = device
            .default_output_config()
            .map_err(|e| anyhow::anyhow!("Failed to get output config: {}", e))?;

        log::info!(
            "Audio device: '{}', format: {:?}, sample rate: {}, channels: {}",
            device.name().unwrap_or_else(|_| "unknown".into()),
            config.sample_format(),
            config.sample_rate().0,
            config.channels(),
        );

        let device_sample_rate = config.sample_rate().0;
        let device_channels = config.channels() as usize;

        // Clone all sound data into a map the callback closure will own.
        // Resample to the device's sample rate and upmix mono→stereo as needed.
        let mut captured_sounds: HashMap<AudioHandle, Vec<f32>> = HashMap::new();
        for (handle, data) in sound_registry.iter() {
            let pcm = prepare_pcm(data, device_sample_rate, device_channels);
            captured_sounds.insert(handle, pcm);
        }

        let (sender, receiver) = mpsc::channel::<AudioCommand>();

        let mut playing: Vec<PlayingSound> = Vec::new();
        let mut master_volume: f32 = 1.0;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |output: &mut [f32], _info| {
                    // Drain incoming commands (non-blocking).
                    while let Ok(cmd) = receiver.try_recv() {
                        process_command(cmd, &mut playing, &mut master_volume);
                    }

                    // Zero the output buffer.
                    for s in output.iter_mut() {
                        *s = 0.0;
                    }

                    // Mix active sounds.
                    for ps in &mut playing {
                        if let Some(src) = captured_sounds.get(&ps.handle) {
                            mix_sound(ps, src, output, master_volume, device_channels);
                        }
                    }

                    // Drop finished sounds.
                    playing.retain(|ps| !ps.finished);
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| anyhow::anyhow!("Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| anyhow::anyhow!("Failed to start audio stream: {}", e))?;

        Ok(AudioSystem {
            _stream: stream,
            sender,
        })
    }

    pub fn sender(&self) -> &mpsc::Sender<AudioCommand> {
        &self.sender
    }
}

/// Resample `data` to `target_rate` and upmix to `target_channels`.
/// Returns interleaved stereo (or mono, if target_channels == 1) f32 samples.
fn prepare_pcm(data: &SoundData, target_rate: u32, target_channels: usize) -> Vec<f32> {
    let src_channels = data.channels as usize;
    let src_rate = data.sample_rate;

    // Step 1: Convert to stereo if needed.
    let stereo: Vec<f32> = if src_channels == 1 {
        // Duplicate mono → stereo
        data.samples.iter().flat_map(|&s| [s, s]).collect()
    } else {
        data.samples.clone()
    };

    // Step 2: Resample if sample rates differ (linear interpolation).
    let src_frames = stereo.len() / 2; // stereo frames
    if src_rate == target_rate {
        if target_channels == 1 {
            // Downmix to mono (average L+R)
            stereo.chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect()
        } else {
            stereo
        }
    } else {
        let ratio = src_rate as f64 / target_rate as f64;
        let target_frames = (src_frames as f64 / ratio).ceil() as usize;
        let mut out = Vec::with_capacity(target_frames * target_channels.max(2));

        for i in 0..target_frames {
            let src_pos = i as f64 * ratio;
            let idx0 = src_pos as usize;
            let idx1 = (idx0 + 1).min(src_frames.saturating_sub(1));
            let frac = src_pos - idx0 as f64;

            let l = lerp(stereo[idx0 * 2], stereo[idx1 * 2], frac as f32);
            let r = lerp(stereo[idx0 * 2 + 1], stereo[idx1 * 2 + 1], frac as f32);

            if target_channels == 1 {
                out.push((l + r) * 0.5);
            } else {
                out.push(l);
                out.push(r);
            }
        }
        out
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Process one `AudioCommand`, mutating `playing` and `master_volume`.
fn process_command(cmd: AudioCommand, playing: &mut Vec<PlayingSound>, master_volume: &mut f32) {
    match cmd {
        AudioCommand::Play {
            handle,
            volume,
            looping,
        } => {
            playing.push(PlayingSound {
                handle,
                cursor: 0,
                volume,
                looping,
                finished: false,
            });
        }
        AudioCommand::Stop { handle } => {
            for ps in &mut *playing {
                if ps.handle == handle {
                    ps.finished = true;
                }
            }
        }
        AudioCommand::StopAll => {
            for ps in &mut *playing {
                ps.finished = true;
            }
        }
        AudioCommand::SetMasterVolume(v) => {
            *master_volume = v.clamp(0.0, 1.0);
        }
    }
}

/// Mix `ps` into `output` from `src`, advancing the cursor.
fn mix_sound(
    ps: &mut PlayingSound,
    src: &[f32],
    output: &mut [f32],
    master_volume: f32,
    channels: usize,
) {
    let gain = ps.volume * master_volume;
    let step = channels; // samples per frame in the output buffer

    let mut out_idx = 0;
    while out_idx < output.len() {
        if ps.cursor >= src.len() {
            if ps.looping {
                ps.cursor = 0;
            } else {
                ps.finished = true;
                break;
            }
        }
        // Mix one frame
        for c in 0..step {
            if ps.cursor + c < src.len() {
                output[out_idx + c] += src[ps.cursor + c] * gain;
            }
        }
        ps.cursor += step;
        out_idx += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten_core::assets::AudioHandle;
    use tungsten_core::audio::AudioCommand;

    fn make_playing(handle: AudioHandle) -> PlayingSound {
        PlayingSound {
            handle,
            cursor: 0,
            volume: 1.0,
            looping: false,
            finished: false,
        }
    }

    #[test]
    fn process_play_adds_entry() {
        let mut playing = Vec::new();
        let mut master = 1.0f32;
        process_command(
            AudioCommand::Play {
                handle: AudioHandle(0),
                volume: 0.5,
                looping: true,
            },
            &mut playing,
            &mut master,
        );
        assert_eq!(playing.len(), 1);
        assert_eq!(playing[0].handle, AudioHandle(0));
        assert!(playing[0].looping);
    }

    #[test]
    fn process_stop_marks_finished() {
        let mut playing = vec![make_playing(AudioHandle(1))];
        let mut master = 1.0f32;
        process_command(
            AudioCommand::Stop {
                handle: AudioHandle(1),
            },
            &mut playing,
            &mut master,
        );
        assert!(playing[0].finished);
    }

    #[test]
    fn process_stop_all_marks_all_finished() {
        let mut playing = vec![make_playing(AudioHandle(0)), make_playing(AudioHandle(1))];
        let mut master = 1.0f32;
        process_command(AudioCommand::StopAll, &mut playing, &mut master);
        assert!(playing.iter().all(|ps| ps.finished));
    }

    #[test]
    fn process_set_master_volume() {
        let mut playing = Vec::new();
        let mut master = 1.0f32;
        process_command(
            AudioCommand::SetMasterVolume(0.3),
            &mut playing,
            &mut master,
        );
        assert!((master - 0.3).abs() < 1e-6);
    }

    #[test]
    fn prepare_pcm_upmix_mono_to_stereo() {
        let data = SoundData {
            samples: vec![0.5, 0.6, 0.7],
            sample_rate: 44100,
            channels: 1,
        };
        let out = prepare_pcm(&data, 44100, 2);
        // Each mono sample should appear twice (L and R)
        assert_eq!(out.len(), 6);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn prepare_pcm_no_resample_passthrough() {
        let data = SoundData {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            sample_rate: 44100,
            channels: 2,
        };
        let out = prepare_pcm(&data, 44100, 2);
        assert_eq!(out.len(), 4);
    }
}
