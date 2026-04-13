use std::collections::HashMap;
use std::path::Path;

/// Opaque handle to a registered sound asset. Keyed by the same u32 scheme
/// as `TextureHandle` — core allocates IDs; `tungsten` stores the decoded PCM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioHandle(pub u32);

/// Decoded PCM audio data. Stored fully in RAM at load time (eager decode).
/// Samples are interleaved stereo f32 values at the file's native sample rate.
/// The audio thread resamples to the device rate at init time if needed.
pub struct SoundData {
    /// Interleaved stereo PCM samples (L, R, L, R, …).
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Number of channels in the original file (1 = mono, 2 = stereo).
    pub channels: u16,
}

impl SoundData {
    /// Decode an audio file (OGG, WAV, MP3) to raw PCM via symphonia.
    pub fn decode(path: &Path) -> anyhow::Result<SoundData> {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let file = std::fs::File::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", path.display(), e))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to probe '{}': {}", path.display(), e))?;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow::anyhow!("No audio track in '{}'", path.display()))?;

        let track_id = track.id;
        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| anyhow::anyhow!("Unknown sample rate in '{}'", path.display()))?;
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| anyhow::anyhow!("Failed to create decoder: {}", e))?;

        let mut all_samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(_)) => break,
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("Decode error: {}", e)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                    sample_buf.copy_interleaved_ref(decoded);
                    all_samples.extend_from_slice(sample_buf.samples());
                }
                Err(symphonia::core::errors::Error::IoError(_)) => break,
                Err(symphonia::core::errors::Error::DecodeError(e)) => {
                    log::warn!("Decode warning in '{}': {}", path.display(), e);
                }
                Err(e) => return Err(anyhow::anyhow!("Decode error: {}", e)),
            }
        }

        Ok(SoundData {
            samples: all_samples,
            sample_rate,
            channels,
        })
    }
}

/// Registry of decoded sound assets. Stored as a Resource in the World.
/// Allocates `AudioHandle`s and stores `SoundData` by handle.
pub struct SoundRegistry {
    next_id: u32,
    sounds: HashMap<AudioHandle, SoundData>,
    id_map: HashMap<String, AudioHandle>,
}

impl SoundRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            sounds: HashMap::new(),
            id_map: HashMap::new(),
        }
    }

    /// Register a decoded sound and return its handle.
    pub fn register(&mut self, id: String, data: SoundData) -> AudioHandle {
        let handle = AudioHandle(self.next_id);
        self.next_id += 1;
        self.id_map.insert(id, handle);
        self.sounds.insert(handle, data);
        handle
    }

    pub fn get(&self, handle: AudioHandle) -> Option<&SoundData> {
        self.sounds.get(&handle)
    }

    pub fn get_by_id(&self, id: &str) -> Option<AudioHandle> {
        self.id_map.get(id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (AudioHandle, &SoundData)> {
        self.sounds.iter().map(|(&h, d)| (h, d))
    }
}

impl Default for SoundRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_allocation_produces_distinct_ids() {
        let mut reg = SoundRegistry::new();
        let data1 = SoundData {
            samples: vec![],
            sample_rate: 44100,
            channels: 2,
        };
        let data2 = SoundData {
            samples: vec![],
            sample_rate: 44100,
            channels: 2,
        };
        let h1 = reg.register("sfx_a".into(), data1);
        let h2 = reg.register("sfx_b".into(), data2);
        assert_ne!(h1, h2);
    }

    #[test]
    fn get_returns_none_for_unknown_handle() {
        let reg = SoundRegistry::new();
        assert!(reg.get(AudioHandle(99)).is_none());
    }

    #[test]
    fn get_returns_data_for_registered_handle() {
        let mut reg = SoundRegistry::new();
        let data = SoundData {
            samples: vec![0.0, 0.0],
            sample_rate: 44100,
            channels: 2,
        };
        let handle = reg.register("sfx_blip".into(), data);
        let retrieved = reg.get(handle).unwrap();
        assert_eq!(retrieved.sample_rate, 44100);
        assert_eq!(retrieved.samples.len(), 2);
    }

    #[test]
    fn get_by_id_resolves_string_to_handle() {
        let mut reg = SoundRegistry::new();
        let data = SoundData {
            samples: vec![],
            sample_rate: 48000,
            channels: 1,
        };
        let handle = reg.register("music_main".into(), data);
        assert_eq!(reg.get_by_id("music_main"), Some(handle));
        assert_eq!(reg.get_by_id("nonexistent"), None);
    }
}
