use std::collections::HashMap;
use std::path::Path;

/// Registered sound handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioHandle(pub u32);

/// Eager-decoded PCM audio data.
pub struct SoundData {
    /// Interleaved f32 PCM samples.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Source channel count.
    pub channels: u16,
}

impl SoundData {
    /// Decode audio file to raw PCM via symphonia.
    ///
    /// End of stream ends decoding. A stream that ends mid-packet (truncated
    /// file) keeps the audio decoded so far and logs a warning; any other I/O
    /// failure is an error. Corrupt packets are skipped with a warning.
    pub fn decode(path: &Path) -> anyhow::Result<SoundData> {
        use std::io::ErrorKind;
        use symphonia::core::codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO};
        use symphonia::core::errors::Error;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::formats::probe::Hint;
        use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
        use symphonia::core::meta::MetadataOptions;

        let file = std::fs::File::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", path.display(), e))?;

        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to probe '{}': {}", path.display(), e))?;

        let (track_id, params) = format
            .tracks()
            .iter()
            .find_map(|t| {
                let audio = t.codec_params.as_ref()?.audio()?;
                (audio.codec != CODEC_ID_NULL_AUDIO).then(|| (t.id, audio.clone()))
            })
            .ok_or_else(|| anyhow::anyhow!("No audio track in '{}'", path.display()))?;

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| anyhow::anyhow!("Unknown sample rate in '{}'", path.display()))?;
        let channels = params.channels.as_ref().map_or(2, |c| c.count() as u16);

        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(|e| anyhow::anyhow!("Failed to create decoder: {e}"))?;

        let mut all_samples: Vec<f32> = Vec::new();
        // Reused per packet; each copy resizes it to that packet's samples.
        let mut packet_samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => break,
                Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    log::warn!(
                        "'{}' is truncated; keeping the audio decoded so far",
                        path.display()
                    );
                    break;
                }
                Err(Error::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("Decode error: {e}")),
            };

            if packet.track_id != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    decoded.copy_to_vec_interleaved(&mut packet_samples);
                    all_samples.extend_from_slice(&packet_samples);
                }
                Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    log::warn!(
                        "'{}' is truncated; keeping the audio decoded so far",
                        path.display()
                    );
                    break;
                }
                // Policy: a corrupt packet is skipped, not fatal.
                Err(Error::DecodeError(e)) => {
                    log::warn!("Decode warning in '{}': {}", path.display(), e);
                }
                Err(e) => return Err(anyhow::anyhow!("Decode error: {e}")),
            }
        }

        Ok(SoundData {
            samples: all_samples,
            sample_rate,
            channels,
        })
    }
}

/// Decoded sound registry resource.
pub struct SoundRegistry {
    next_id: u32,
    sounds: HashMap<AudioHandle, SoundData>,
    id_map: HashMap<String, AudioHandle>,
    /// Manifest `(volume, looping)` defaults.
    defaults: HashMap<AudioHandle, (f32, bool)>,
}

impl SoundRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 0,
            sounds: HashMap::new(),
            id_map: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    /// Register decoded sound and defaults.
    pub fn register(
        &mut self,
        id: String,
        data: SoundData,
        volume: f32,
        looping: bool,
    ) -> AudioHandle {
        let handle = AudioHandle(self.next_id);
        self.next_id += 1;
        self.id_map.insert(id, handle);
        self.sounds.insert(handle, data);
        self.defaults.insert(handle, (volume, looping));
        handle
    }

    #[must_use]
    pub fn get(&self, handle: AudioHandle) -> Option<&SoundData> {
        self.sounds.get(&handle)
    }

    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<AudioHandle> {
        self.id_map.get(id).copied()
    }

    /// Manifest default volume.
    #[must_use]
    pub fn get_volume(&self, handle: AudioHandle) -> f32 {
        self.defaults.get(&handle).map_or(1.0, |&(v, _)| v)
    }

    /// Manifest default looping flag.
    #[must_use]
    pub fn get_looping(&self, handle: AudioHandle) -> bool {
        self.defaults.get(&handle).is_some_and(|&(_, l)| l)
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
#[path = "../tests/assets/audio.rs"]
mod tests;
