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
