//! Decoder coverage for every enabled Symphonia path (wav, ogg/vorbis, mp3,
//! aac) using the synthetic tones in `tests/fixtures/audio/`: format metadata,
//! interleaving, length, finite PCM, and behavior on corrupt input.
//!
//! These pin current behavior, including one known gap: the workspace enables
//! Symphonia's `wav` demuxer but not its `pcm` codec, so PCM WAV files are
//! rejected with "unsupported codec".

use std::fs;
use std::path::{Path, PathBuf};

use tungsten_core::SoundData;

/// Nominal fixture length in seconds; lossy codecs add priming/padding frames.
const DURATION_SECS: f64 = 0.25;
const TONE_AMPLITUDE: f32 = 0.5;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/audio")
        .join(name)
}

fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("audio_decode");
    fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write scratch file");
    path
}

fn frames(data: &SoundData) -> usize {
    data.samples.len() / usize::from(data.channels)
}

fn channel(data: &SoundData, index: usize) -> impl Iterator<Item = f32> + '_ {
    data.samples
        .iter()
        .copied()
        .skip(index)
        .step_by(usize::from(data.channels))
}

fn rms(samples: impl Iterator<Item = f32>) -> f32 {
    let (sum, n) = samples.fold((0.0_f64, 0_usize), |(s, n), x| {
        (s + f64::from(x) * f64::from(x), n + 1)
    });
    if n == 0 {
        0.0
    } else {
        (sum / n as f64).sqrt() as f32
    }
}

fn check_tone(name: &str, rate: u32, channels: u16, max_extra_frames: usize) -> SoundData {
    let data = SoundData::decode(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let nominal = (f64::from(rate) * DURATION_SECS) as usize;
    let n = frames(&data);
    let peak = channel(&data, 0).fold(0.0_f32, |m, x| m.max(x.abs()));
    eprintln!(
        "{name}: rate={} channels={} frames={n} (nominal {nominal}) left_peak={peak:.3}",
        data.sample_rate, data.channels
    );

    assert_eq!(data.sample_rate, rate, "{name}: sample rate");
    assert_eq!(data.channels, channels, "{name}: channel count");
    assert_eq!(
        data.samples.len() % usize::from(channels),
        0,
        "{name}: whole frames"
    );
    assert!(
        n >= nominal * 9 / 10 && n <= nominal + max_extra_frames,
        "{name}: {n} frames outside [{}, {}]",
        nominal * 9 / 10,
        nominal + max_extra_frames
    );
    assert!(
        data.samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-3),
        "{name}: PCM must be finite and within [-1, 1]"
    );
    assert!(
        // Lossy codecs overshoot the source peak slightly (AAC ~0.63).
        (TONE_AMPLITUDE * 0.8..=TONE_AMPLITUDE * 1.3).contains(&peak),
        "{name}: left peak {peak} not near {TONE_AMPLITUDE}"
    );
    if channels == 2 {
        let left = rms(channel(&data, 0));
        let right = rms(channel(&data, 1));
        eprintln!("{name}: left_rms={left:.4} right_rms={right:.4}");
        assert!(left > 0.25, "{name}: left RMS {left} too low");
        assert!(
            right < 0.02,
            "{name}: right RMS {right}; channels not interleaved L,R"
        );
    }
    data
}

#[test]
fn wav_pcm_is_rejected_without_the_pcm_codec_feature() {
    // Known gap (see module docs). If Symphonia's `pcm` feature is enabled,
    // replace this with `check_tone("tone_44100_stereo.wav", 44_100, 2, 0)`
    // and an exact 11_025-frame assertion.
    let err = SoundData::decode(&fixture("tone_44100_stereo.wav"))
        .err()
        .expect("PCM WAV decodes only with the pcm codec feature");
    assert!(
        err.to_string().contains("unsupported"),
        "unexpected error: {err}"
    );
}

#[test]
fn ogg_vorbis_mono_decodes() {
    check_tone("tone_22050_mono.ogg", 22_050, 1, 2_048);
}

#[test]
fn mp3_stereo_decodes() {
    check_tone("tone_44100_stereo.mp3", 44_100, 2, 4_096);
}

#[test]
fn aac_adts_stereo_decodes() {
    check_tone("tone_48000_stereo.aac", 48_000, 2, 4_096);
}

/// Decode `name` cut to `keep` of its bytes; the audio data must come out shorter.
fn check_truncated(name: &str, keep: fn(usize) -> usize) {
    let bytes = fs::read(fixture(name)).expect("read fixture");
    let full = SoundData::decode(&fixture(name)).expect("full decode");
    let ext = Path::new(name).extension().unwrap().to_str().unwrap();
    let cut = scratch(&format!("truncated.{ext}"), &bytes[..keep(bytes.len())]);
    let data = SoundData::decode(&cut).unwrap_or_else(|e| panic!("truncated {name}: {e}"));
    eprintln!(
        "truncated {name}: {} of {} frames",
        frames(&data),
        frames(&full)
    );
    assert!(
        frames(&data) < frames(&full),
        "truncated {name} decoded as long as the full file"
    );
    assert!(
        data.samples.iter().all(|s| s.is_finite()),
        "truncated {name}: non-finite PCM"
    );
}

#[test]
fn truncated_mp3_decodes_the_available_prefix() {
    check_truncated("tone_44100_stereo.mp3", |len| len / 2);
}

#[test]
fn truncated_ogg_is_an_error() {
    // The Ogg reader scans to the final page while probing, so a stream cut
    // mid-page fails whether the cut lands in the headers or the audio pages.
    let bytes = fs::read(fixture("tone_22050_mono.ogg")).expect("read fixture");
    for (label, keep) in [
        ("header", bytes.len() / 2),
        ("audio", bytes.len() - bytes.len() / 20),
    ] {
        let cut = scratch(&format!("truncated-{label}.ogg"), &bytes[..keep]);
        let err = SoundData::decode(&cut).err();
        eprintln!("truncated ogg ({label}, {keep} bytes): {err:?}");
        assert!(err.is_some(), "Ogg cut in its {label} must not decode");
    }
}

#[test]
fn garbage_empty_and_missing_inputs_are_errors() {
    let garbage = scratch("garbage.wav", &[0x5a; 512]);
    assert!(
        SoundData::decode(&garbage).is_err(),
        "garbage bytes must not decode"
    );

    let empty = scratch("empty.ogg", &[]);
    assert!(
        SoundData::decode(&empty).is_err(),
        "empty file must not decode"
    );

    let missing = fixture("does-not-exist.wav");
    let err = SoundData::decode(&missing)
        .err()
        .expect("missing file must fail");
    assert!(
        err.to_string().contains("Failed to open"),
        "unexpected error: {err}"
    );
}
