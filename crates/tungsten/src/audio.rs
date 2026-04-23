use std::collections::HashMap;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::RingBuffer;
use tungsten_core::assets::{AudioHandle, SoundData, SoundRegistry};
use tungsten_core::audio::AudioCommand;

/// D-034 SPSC command ring capacity.
const CMD_RING_CAPACITY: usize = 64;

struct PlayingSound {
    handle: AudioHandle,
    /// Interleaved sample cursor.
    cursor: usize,
    /// Per-instance volume.
    volume: f32,
    looping: bool,
    finished: bool,
}

/// Audio subsystem: live cpal stream plus command producer.
pub struct AudioSystem {
    /// Keep cpal stream alive.
    _stream: cpal::Stream,
    producer: rtrb::Producer<AudioCommand>,
}

impl AudioSystem {
    /// Initialize output stream and callback-owned sound data.
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

        // Callback owns cloned/resampled PCM.
        let mut captured_sounds: HashMap<AudioHandle, Vec<f32>> = HashMap::new();
        for (handle, data) in sound_registry.iter() {
            let pcm = prepare_pcm(data, device_sample_rate, device_channels);
            captured_sounds.insert(handle, pcm);
        }

        let (producer, mut consumer) = RingBuffer::<AudioCommand>::new(CMD_RING_CAPACITY);

        let mut playing: Vec<PlayingSound> = Vec::new();
        let mut master_volume: f32 = 1.0;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |output: &mut [f32], _info| {
                    // D-034: wait-free, allocation-free callback command drain.
                    while let Ok(cmd) = consumer.pop() {
                        process_command(cmd, &mut playing, &mut master_volume);
                    }

                    for s in output.iter_mut() {
                        *s = 0.0;
                    }

                    for ps in &mut playing {
                        if let Some(src) = captured_sounds.get(&ps.handle) {
                            mix_sound(ps, src, output, master_volume, device_channels);
                        }
                    }

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
            producer,
        })
    }

    /// Send command to callback thread; full ring drops command.
    pub fn send(&mut self, cmd: AudioCommand) {
        let _ = self.producer.push(cmd);
    }
}

/// Resample/upmix PCM to device format.
fn prepare_pcm(data: &SoundData, target_rate: u32, target_channels: usize) -> Vec<f32> {
    let src_channels = data.channels as usize;
    let src_rate = data.sample_rate;

    let stereo: Vec<f32> = if src_channels == 1 {
        data.samples.iter().flat_map(|&s| [s, s]).collect()
    } else {
        data.samples.clone()
    };

    let src_frames = stereo.len() / 2;
    if src_rate == target_rate {
        if target_channels == 1 {
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

fn mix_sound(
    ps: &mut PlayingSound,
    src: &[f32],
    output: &mut [f32],
    master_volume: f32,
    channels: usize,
) {
    let gain = ps.volume * master_volume;
    let step = channels;

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
#[path = "tests/audio.rs"]
mod tests;
