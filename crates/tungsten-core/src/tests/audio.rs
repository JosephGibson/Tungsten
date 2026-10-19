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
            fade,
            ..
        } => {
            assert_eq!(*handle, AudioHandle(5));
            assert!((volume - 1.0).abs() < 1e-6);
            assert!(!looping);
            assert_eq!(*fade, 0.0);
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

#[test]
fn a_volume_that_is_not_finite_or_is_negative_plays_silent() {
    // A slider computing `x / 0.0` sent NaN, and every output sample went
    // NaN; a negative gain inverts the waveform.
    let mut cmds = AudioCommands::new();
    for volume in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.5] {
        cmds.play_with(AudioHandle(0), volume, false);
        cmds.set_master_volume(volume);
    }
    let expected = [0.0, 0.0, 0.0, 0.0, 0.5];
    let drained = cmds.drain();
    for (pair, want) in drained.chunks(2).zip(expected) {
        match pair {
            [
                AudioCommand::Play { volume, .. },
                AudioCommand::SetMasterVolume(master),
            ] => {
                assert_eq!(*volume, want);
                assert_eq!(*master, want);
            }
            other => panic!("unexpected commands {other:?}"),
        }
    }
}

/// The voice and fade of each `Play` in `commands`, in order.
fn plays(commands: &[AudioCommand]) -> Vec<(Voice, f32)> {
    commands
        .iter()
        .filter_map(|command| match command {
            AudioCommand::Play { voice, fade, .. } => Some((*voice, *fade)),
            _ => None,
        })
        .collect()
}

#[test]
fn voices_count_from_1_in_order_and_each_play_carries_its_own_with_no_fade() {
    let mut cmds = AudioCommands::new();
    let first = cmds.play(AudioHandle(0));
    cmds.stop(AudioHandle(0));
    let second = cmds.play_looping(AudioHandle(1));
    let third = cmds.play_with(AudioHandle(2), 0.5, false);
    assert_eq!([first, second, third], [Voice(1), Voice(2), Voice(3)]);
    assert_eq!(
        plays(&cmds.drain()),
        [(first, 0.0), (second, 0.0), (third, 0.0)]
    );
    // Draining does not restart the count.
    assert_eq!(cmds.play(AudioHandle(0)), Voice(4));
}

#[test]
fn a_replaced_queue_numbers_its_voices_from_1_again() {
    let mut old = AudioCommands::new();
    old.play(AudioHandle(0));
    old.play(AudioHandle(0));
    assert_eq!(AudioCommands::new().play(AudioHandle(0)), Voice(1));
    assert_eq!(AudioCommands::default().play(AudioHandle(0)), Voice(1));
}

#[test]
fn play_faded_queues_one_play_carrying_its_fade() {
    let mut cmds = AudioCommands::new();
    let voice = cmds.play_faded(AudioHandle(7), 0.8, true, 2.0);
    let drained = cmds.drain();
    assert_eq!(drained.len(), 1);
    match &drained[0] {
        AudioCommand::Play {
            handle,
            voice: played,
            volume,
            looping,
            fade,
        } => {
            assert_eq!((*handle, *played), (AudioHandle(7), voice));
            assert_eq!((*volume, *looping, *fade), (0.8, true, 2.0));
        }
        other => panic!("unexpected {other:?}"),
    }
    // The fade's volume and seconds keep the queue's rules.
    cmds.play_faded(AudioHandle(7), f32::NAN, false, -1.0);
    cmds.play_faded(AudioHandle(7), -0.5, false, f32::INFINITY);
    for command in cmds.drain() {
        assert!(
            matches!(command, AudioCommand::Play { volume, fade, .. } if volume == 0.0 && fade == 0.0),
            "{command:?}"
        );
    }
}

#[test]
fn voice_and_bus_commands_sanitize_volumes_and_fades() {
    let mut cmds = AudioCommands::new();
    let voice = cmds.play(AudioHandle(0));
    cmds.drain();
    for seconds in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
        cmds.stop_voice(voice, seconds);
        cmds.set_voice_volume(voice, f32::NAN, seconds);
    }
    cmds.stop_voice(voice, 0.5);
    cmds.set_voice_volume(voice, 1.5, 0.25);
    for volume in [f32::NAN, -1.0, 2.0, f32::INFINITY, 0.25] {
        cmds.set_bus_volume(AudioBus::Music, volume);
    }
    let expected = std::iter::repeat_n(
        [
            AudioCommand::StopVoice { voice, fade: 0.0 },
            AudioCommand::SetVoiceVolume {
                voice,
                volume: 0.0,
                fade: 0.0,
            },
        ],
        4,
    )
    .flatten()
    .chain([
        AudioCommand::StopVoice { voice, fade: 0.5 },
        // A voice may play above 1; only a bus is capped.
        AudioCommand::SetVoiceVolume {
            voice,
            volume: 1.5,
            fade: 0.25,
        },
    ])
    .chain(
        [0.0, 0.0, 1.0, 0.0, 0.25].map(|volume| AudioCommand::SetBusVolume {
            bus: AudioBus::Music,
            volume,
        }),
    )
    .collect::<Vec<_>>();
    assert_eq!(cmds.drain(), expected);
}

#[test]
fn audio_bus_all_names_and_default() {
    assert_eq!(AudioBus::ALL, [AudioBus::Music, AudioBus::Effects]);
    assert_eq!(
        AudioBus::ALL
            .iter()
            .map(|bus| bus.name())
            .collect::<Vec<_>>(),
        ["music", "effects"]
    );
    assert_eq!(AudioBus::default(), AudioBus::Effects);
    // The names are the serde names the manifest and the settings use.
    for &bus in AudioBus::ALL {
        let json = serde_json::to_value(bus).unwrap();
        assert_eq!(json, serde_json::json!(bus.name()));
        assert_eq!(serde_json::from_value::<AudioBus>(json).unwrap(), bus);
    }
    assert!(serde_json::from_value::<AudioBus>(serde_json::json!("voice")).is_err());
}
