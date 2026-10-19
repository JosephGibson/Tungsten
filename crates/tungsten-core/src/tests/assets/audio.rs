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
    let h1 = reg.register("sfx_a".into(), data1, 1.0, false);
    let h2 = reg.register("sfx_b".into(), data2, 1.0, false);
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
    let handle = reg.register("sfx_blip".into(), data, 0.8, false);
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
    let handle = reg.register("music_main".into(), data, 0.4, true);
    assert_eq!(reg.get_by_id("music_main"), Some(handle));
    assert_eq!(reg.get_by_id("nonexistent"), None);
}

#[test]
fn manifest_defaults_roundtrip() {
    let mut reg = SoundRegistry::new();
    let sfx = reg.register(
        "sfx_blip".into(),
        SoundData {
            samples: vec![],
            sample_rate: 44100,
            channels: 2,
        },
        0.8,
        false,
    );
    let music = reg.register(
        "music_main".into(),
        SoundData {
            samples: vec![],
            sample_rate: 44100,
            channels: 2,
        },
        0.4,
        true,
    );
    assert!((reg.get_volume(sfx) - 0.8).abs() < 1e-6);
    assert!(!reg.get_looping(sfx));
    assert!((reg.get_volume(music) - 0.4).abs() < 1e-6);
    assert!(reg.get_looping(music));
    assert!((reg.get_volume(AudioHandle(99)) - 1.0).abs() < 1e-6);
    assert!(!reg.get_looping(AudioHandle(99)));
}

fn silence() -> SoundData {
    SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    }
}

#[test]
fn a_sound_registered_in_code_plays_on_effects_with_64_voices() {
    let mut reg = SoundRegistry::new();
    let handle = reg.register("sfx".into(), silence(), 1.0, false);
    assert_eq!(reg.get_bus(handle), AudioBus::Effects);
    assert_eq!(reg.get_max_voices(handle), MAX_VOICES);
    assert_eq!(MAX_VOICES, 64);
    // An unknown handle reads the same defaults.
    assert_eq!(reg.get_bus(AudioHandle(99)), AudioBus::Effects);
    assert_eq!(reg.get_max_voices(AudioHandle(99)), 64);
}

#[test]
fn the_registry_keeps_a_bus_and_a_voice_limit_per_sound_clamped() {
    let mut reg = SoundRegistry::new();
    let music = reg.register("music".into(), silence(), 0.4, true);
    let hit = reg.register("hit".into(), silence(), 0.8, false);
    reg.set_bus(music, AudioBus::Music);
    reg.set_max_voices(hit, 4);
    assert_eq!(
        (reg.get_bus(music), reg.get_max_voices(music)),
        (AudioBus::Music, 64)
    );
    assert_eq!(
        (reg.get_bus(hit), reg.get_max_voices(hit)),
        (AudioBus::Effects, 4)
    );
    // The volume and the loop flag stay as registered.
    assert!(reg.get_looping(music) && (reg.get_volume(hit) - 0.8).abs() < 1e-6);
    for (limit, kept) in [(0, 1), (1, 1), (64, 64), (65, 64), (u32::MAX, 64)] {
        reg.set_max_voices(hit, limit);
        assert_eq!(reg.get_max_voices(hit), kept, "{limit}");
    }
    // A handle never registered takes nothing.
    reg.set_bus(AudioHandle(99), AudioBus::Music);
    reg.set_max_voices(AudioHandle(99), 2);
    assert_eq!(reg.get_bus(AudioHandle(99)), AudioBus::Effects);
    assert_eq!(reg.get_max_voices(AudioHandle(99)), 64);
}
