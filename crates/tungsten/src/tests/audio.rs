use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tungsten_core::AudioCommands;
use tungsten_core::assets::AudioHandle;
use tungsten_core::audio::AudioCommand;

/// The tests' device rate: 10 ms is 10 frames.
const RATE: u32 = 1_000;

/// A sound of `frames` frames, each `left` and `right`, on `bus` with
/// `limit` voices.
fn sound(frames: usize, left: f32, right: f32, bus: AudioBus, limit: u32) -> MixSound {
    MixSound {
        pcm: [left, right].repeat(frames),
        bus: bus_index(bus),
        limit,
    }
}

/// A second of 1.0 on both sides, on effects, with `limit` voices.
fn tone(limit: u32) -> MixSound {
    sound(1_000, 1.0, 1.0, AudioBus::Effects, limit)
}

fn mixer_of(sounds: Vec<MixSound>, channels: usize) -> Mixer {
    Mixer::new(sounds.into_iter().map(Some).collect(), RATE, channels)
}

fn apply_all(mixer: &mut Mixer, commands: &[AudioCommand]) {
    for command in commands {
        mixer.apply(command);
    }
}

/// `frames` frames rendered, interleaved.
fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * mixer.channels];
    mixer.render(&mut out);
    out
}

/// One channel of an interleaved buffer.
fn channel(out: &[f32], channels: usize, channel: usize) -> Vec<f32> {
    out.iter()
        .skip(channel)
        .step_by(channels)
        .copied()
        .collect()
}

/// The left channel of `frames` stereo frames.
fn lefts(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    channel(&render(mixer, frames), 2, 0)
}

/// The mixer's voices, oldest first: the voice, whether it loops and
/// whether it is stopping.
fn voices(mixer: &Mixer) -> Vec<(Voice, bool, bool)> {
    let mut held: Vec<&PlayingVoice> = mixer.voices.iter().flatten().collect();
    held.sort_by_key(|voice| voice.started);
    held.iter()
        .map(|voice| (voice.voice, voice.looping, voice.stopping))
        .collect()
}

/// The mixer's playing loops, oldest first.
fn playing_loops(mixer: &Mixer) -> Vec<Voice> {
    voices(mixer)
        .into_iter()
        .filter(|&(_, looping, stopping)| looping && !stopping)
        .map(|(voice, _, _)| voice)
        .collect()
}

/// A play's handle, voice, volume, loop flag and fade.
fn play_parts(command: &AudioCommand) -> Option<(AudioHandle, Voice, f32, bool, f32)> {
    match *command {
        AudioCommand::Play {
            handle,
            voice,
            volume,
            looping,
            fade,
            ..
        } => Some((handle, voice, volume, looping, fade)),
        _ => None,
    }
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0e-5,
        "{actual} is not {expected}"
    );
}

#[test]
fn prepare_pcm_upmixes_mono_to_stereo() {
    let data = SoundData {
        samples: vec![0.5, 0.6, 0.7],
        sample_rate: 44100,
        channels: 1,
    };
    assert_eq!(prepare_pcm(&data, 44100), [0.5, 0.5, 0.6, 0.6, 0.7, 0.7]);
}

#[test]
fn prepare_pcm_passes_stereo_through_at_its_rate() {
    let data = SoundData {
        samples: vec![0.1, 0.2, 0.3, 0.4],
        sample_rate: 44100,
        channels: 2,
    };
    assert_eq!(prepare_pcm(&data, 44100), [0.1, 0.2, 0.3, 0.4]);
}

#[test]
fn prepare_pcm_takes_a_wider_sources_first_two_channels() {
    let data = SoundData {
        samples: vec![0.1, 0.2, 9.0, 9.0, 0.3, 0.4, 9.0, 9.0],
        sample_rate: 48_000,
        channels: 4,
    };
    assert_eq!(prepare_pcm(&data, 48_000), [0.1, 0.2, 0.3, 0.4]);
    // Resampled to half the rate: one frame in two.
    assert_eq!(prepare_pcm(&data, 24_000), [0.1, 0.2]);
}

#[test]
fn a_voice_plays_at_its_volume_times_its_bus_times_master() {
    // A music sound in the left channel, an effects sound in the right.
    let mut mixer = mixer_of(
        vec![
            sound(1_000, 0.5, 0.0, AudioBus::Music, 64),
            sound(1_000, 0.0, 0.5, AudioBus::Effects, 64),
        ],
        2,
    );
    let mut audio = AudioCommands::new();
    audio.play_with(AudioHandle(0), 0.8, true);
    audio.play_with(AudioHandle(1), 1.0, true);
    apply_all(&mut mixer, &audio.drain());
    let out = render(&mut mixer, 1);
    assert_close(out[0], 0.4);
    assert_close(out[1], 0.5);
    audio.set_bus_volume(AudioBus::Music, 0.5);
    apply_all(&mut mixer, &audio.drain());
    render(&mut mixer, 10);
    let out = render(&mut mixer, 1);
    assert_close(out[0], 0.2);
    assert_close(out[1], 0.5);
    audio.set_master_volume(0.5);
    apply_all(&mut mixer, &audio.drain());
    render(&mut mixer, 10);
    let out = render(&mut mixer, 1);
    assert_close(out[0], 0.1);
    assert_close(out[1], 0.25);
}

#[test]
fn a_fade_in_starts_at_0_and_reaches_its_volume_in_exactly_its_frames() {
    let mut mixer = mixer_of(vec![tone(64)], 2);
    let mut audio = AudioCommands::new();
    audio.play_faded(AudioHandle(0), 0.8, true, 0.1);
    apply_all(&mut mixer, &audio.drain());
    let left = lefts(&mut mixer, 120);
    assert_eq!(left[0], 0.0);
    // Linear: the midpoint is half the volume.
    assert_close(left[50], 0.4);
    for (frame, &sample) in left.iter().enumerate().take(100) {
        assert_close(sample, 0.8 * frame as f32 / 100.0);
    }
    assert!(left[99] < 0.8);
    assert!(left[100..].iter().all(|&sample| sample == 0.8));
}

#[test]
fn a_stop_fades_the_voice_out_ends_it_and_nothing_revives_it() {
    let mut mixer = mixer_of(vec![tone(64)], 2);
    let mut audio = AudioCommands::new();
    let voice = audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    render(&mut mixer, 5);
    audio.stop_voice(voice, 0.05);
    apply_all(&mut mixer, &audio.drain());
    let left = lefts(&mut mixer, 60);
    assert_eq!(left[0], 1.0);
    assert_close(left[25], 0.5);
    assert!(left[49] > 0.0);
    assert!(left[50..].iter().all(|&sample| sample == 0.0));
    assert!(mixer.voices.iter().all(Option::is_none), "the slot is free");
    audio.set_voice_volume(voice, 1.0, 0.0);
    audio.stop_voice(voice, 1.0);
    apply_all(&mut mixer, &audio.drain());
    assert!(render(&mut mixer, 10).iter().all(|&sample| sample == 0.0));
    // A stop without a fade takes 10 ms.
    let voice = audio.play(AudioHandle(0));
    audio.stop_voice(voice, 0.0);
    apply_all(&mut mixer, &audio.drain());
    let left = lefts(&mut mixer, 12);
    assert_close(left[5], 0.5);
    assert!(left[10..].iter().all(|&sample| sample == 0.0));
}

#[test]
fn stop_and_stop_all_fade_over_10_ms() {
    for stop_all in [false, true] {
        let mut mixer = mixer_of(vec![tone(64)], 2);
        let mut audio = AudioCommands::new();
        audio.play_looping(AudioHandle(0));
        if stop_all {
            audio.stop_all();
        } else {
            audio.stop(AudioHandle(0));
        }
        apply_all(&mut mixer, &audio.drain());
        let left = lefts(&mut mixer, 12);
        assert_eq!(left[0], 1.0);
        assert_close(left[5], 0.5);
        assert!(left[10..].iter().all(|&sample| sample == 0.0));
        assert!(mixer.voices.iter().all(Option::is_none));
    }
}

#[test]
fn master_and_bus_changes_ramp_over_10_ms() {
    let mut mixer = mixer_of(vec![tone(64)], 2);
    let mut audio = AudioCommands::new();
    audio.play_looping(AudioHandle(0));
    audio.set_master_volume(0.0);
    apply_all(&mut mixer, &audio.drain());
    let left = lefts(&mut mixer, 12);
    assert_eq!(left[0], 1.0);
    assert_close(left[5], 0.5);
    assert!(left[10..].iter().all(|&sample| sample == 0.0));
    audio.set_master_volume(1.0);
    audio.set_bus_volume(AudioBus::Effects, 0.5);
    apply_all(&mut mixer, &audio.drain());
    let left = lefts(&mut mixer, 12);
    // Both ramps run at once: frame 5 is 0.5 × 0.75.
    assert_eq!(left[0], 0.0);
    assert_close(left[5], 0.5 * 0.75);
    assert!(left[10..].iter().all(|&sample| sample == 0.5));
}

#[test]
fn a_sound_at_its_limit_loses_its_oldest_one_shot_then_its_oldest_loop() {
    // Sound 0 takes two voices; sound 1's voice is never touched.
    let mut mixer = mixer_of(vec![tone(2), tone(64)], 2);
    let mut audio = AudioCommands::new();
    let other = audio.play_looping(AudioHandle(1));
    audio.play(AudioHandle(0));
    let shot_2 = audio.play(AudioHandle(0));
    let loop_3 = audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(
        voices(&mixer),
        [
            (other, true, false),
            (shot_2, false, false),
            (loop_3, true, false)
        ],
        "the oldest one-shot ended for the loop"
    );
    audio.play(AudioHandle(0));
    let loop_5 = audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(
        voices(&mixer),
        [
            (other, true, false),
            (loop_3, true, false),
            (loop_5, true, false)
        ],
        "one-shots go before loops"
    );
    // Only loops of it play: the oldest loop ends.
    let shot_6 = audio.play(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(
        voices(&mixer),
        [
            (other, true, false),
            (loop_5, true, false),
            (shot_6, false, false)
        ]
    );
    // A voice fading after a stop counts toward no limit.
    audio.stop_voice(loop_5, 1.0);
    let shot_7 = audio.play(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(
        voices(&mixer),
        [
            (other, true, false),
            (loop_5, true, true),
            (shot_6, false, false),
            (shot_7, false, false)
        ]
    );
}

#[test]
fn with_every_slot_busy_a_play_takes_a_fading_slot_then_the_oldest_one_shot_and_64_loops_drop_it() {
    // Loops of sound 0 and one-shots of sound 1 fill the 64 slots.
    let mut mixer = mixer_of(vec![tone(64), tone(64), tone(64)], 2);
    let mut audio = AudioCommands::new();
    let loops: Vec<Voice> = (0..62)
        .map(|_| audio.play_looping(AudioHandle(0)))
        .collect();
    let shots = [audio.play(AudioHandle(1)), audio.play(AudioHandle(1))];
    audio.stop_voice(loops[10], 1.0);
    apply_all(&mut mixer, &audio.drain());
    assert!(mixer.voices.iter().all(Option::is_some));
    let first = audio.play_looping(AudioHandle(2));
    apply_all(&mut mixer, &audio.drain());
    let held = voices(&mixer);
    assert!(
        held.iter().all(|&(_, _, stopping)| !stopping),
        "the fading slot went"
    );
    assert!(held.contains(&(first, true, false)));
    let second = audio.play_looping(AudioHandle(2));
    apply_all(&mut mixer, &audio.drain());
    let held = voices(&mixer);
    assert!(
        !held.iter().any(|&(voice, _, _)| voice == shots[0]),
        "the oldest one-shot went"
    );
    assert!(held.iter().any(|&(voice, _, _)| voice == shots[1]));
    assert!(held.contains(&(second, true, false)));
    audio.play_looping(AudioHandle(2));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(playing_loops(&mixer).len(), 64);
    let before = voices(&mixer);
    let dropped = [
        audio.play_looping(AudioHandle(2)),
        audio.play(AudioHandle(1)),
    ];
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(voices(&mixer), before, "64 loops: {dropped:?} dropped");
}

#[test]
fn commands_for_unknown_voices_and_handles_do_nothing() {
    // Sound 1 has no samples, so it has no entry; sound 7 does not exist.
    let mut mixer = Mixer::new(vec![Some(tone(64)), None], RATE, 2);
    let mut audio = AudioCommands::new();
    let voice = audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    let mut elsewhere = AudioCommands::new();
    let stranger = (0..5).map(|_| elsewhere.play(AudioHandle(0))).last();
    let stranger = stranger.unwrap();
    audio.play(AudioHandle(1));
    audio.play_looping(AudioHandle(7));
    audio.stop(AudioHandle(7));
    audio.stop_voice(stranger, 0.0);
    audio.set_voice_volume(stranger, 0.0, 0.0);
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(voices(&mixer), [(voice, true, false)]);
    assert!(render(&mut mixer, 20).iter().all(|&sample| sample == 1.0));
}

#[test]
fn two_full_scale_voices_stay_within_the_clamp() {
    for level in [1.0, -1.0] {
        let mut mixer = mixer_of(vec![sound(100, level, level, AudioBus::Effects, 64)], 2);
        let mut audio = AudioCommands::new();
        audio.play(AudioHandle(0));
        audio.play_with(AudioHandle(0), 1.0, true);
        apply_all(&mut mixer, &audio.drain());
        assert!(render(&mut mixer, 10).iter().all(|&sample| sample == level));
    }
}

#[test]
fn a_six_channel_device_gets_stereo_in_its_first_two_channels_and_a_mono_one_the_mean() {
    let ramp = MixSound {
        pcm: (0..8)
            .flat_map(|frame| [frame as f32 * 0.1, frame as f32 * 0.3])
            .collect(),
        bus: bus_index(AudioBus::Effects),
        limit: 64,
    };
    let mut six = Mixer::new(vec![Some(ramp.clone())], RATE, 6);
    let mut audio = AudioCommands::new();
    audio.play(AudioHandle(0));
    let commands = audio.drain();
    apply_all(&mut six, &commands);
    let out = render(&mut six, 4);
    for frame in 0..4 {
        // One source frame per device frame.
        assert_close(out[frame * 6], frame as f32 * 0.1);
        assert_close(out[frame * 6 + 1], frame as f32 * 0.3);
        assert!(
            out[frame * 6 + 2..frame * 6 + 6]
                .iter()
                .all(|&sample| sample == 0.0)
        );
    }
    let mut mono = Mixer::new(vec![Some(ramp)], RATE, 1);
    apply_all(&mut mono, &commands);
    let out = render(&mut mono, 4);
    for (frame, &sample) in out.iter().enumerate() {
        assert_close(sample, frame as f32 * 0.2);
    }
}

#[test]
fn plays_and_stops_never_grow_the_voice_table() {
    let mut mixer = mixer_of(vec![tone(64), tone(3)], 2);
    let capacity = mixer.voices.capacity();
    assert_eq!(mixer.voices.len(), 64);
    let mut audio = AudioCommands::new();
    for round in 0..200_u32 {
        let voice = audio.play_looping(AudioHandle(round % 2));
        audio.play(AudioHandle(0));
        if round % 3 == 0 {
            audio.stop_voice(voice, 0.002);
        }
        if round % 50 == 0 {
            audio.stop_all();
        }
        apply_all(&mut mixer, &audio.drain());
        render(&mut mixer, 1);
    }
    assert_eq!(mixer.voices.capacity(), capacity);
    assert_eq!(mixer.voices.len(), 64);
}

#[test]
fn a_loop_wraps_and_a_one_shot_frees_its_slot() {
    let steps = MixSound {
        pcm: vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3],
        bus: bus_index(AudioBus::Effects),
        limit: 64,
    };
    let mut mixer = Mixer::new(vec![Some(steps)], RATE, 2);
    let mut audio = AudioCommands::new();
    audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(lefts(&mut mixer, 7), [0.1, 0.2, 0.3, 0.1, 0.2, 0.3, 0.1]);
    audio.stop_all();
    apply_all(&mut mixer, &audio.drain());
    render(&mut mixer, 20);
    audio.play(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    assert_eq!(lefts(&mut mixer, 5), [0.1, 0.2, 0.3, 0.0, 0.0]);
    assert!(mixer.voices.iter().all(Option::is_none));
}

#[test]
fn raw_commands_with_bad_volumes_or_fades_play_silent_or_at_once() {
    // A command's fields can be written outside `AudioCommands`' checks.
    let mut audio = AudioCommands::new();
    for (volume, fade) in [
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
        (-2.0, 0.0),
        (1.0, f32::NAN),
        (1.0, -1.0),
    ] {
        audio.play(AudioHandle(0));
        let mut command = audio.drain().remove(0);
        if let AudioCommand::Play {
            volume: raw_volume,
            fade: raw_fade,
            ..
        } = &mut command
        {
            *raw_volume = volume;
            *raw_fade = fade;
        }
        let mut mixer = mixer_of(vec![tone(64)], 2);
        mixer.apply(&command);
        let expected = if volume.is_finite() && volume > 0.0 {
            1.0
        } else {
            0.0
        };
        assert_eq!(lefts(&mut mixer, 1), [expected], "{volume} {fade}");
    }
    let mut mixer = mixer_of(vec![tone(64)], 2);
    for (raw, kept) in [(f32::NAN, 0.0), (-1.0, 0.0), (2.0, 1.0)] {
        mixer.apply(&AudioCommand::SetMasterVolume(raw));
        render(&mut mixer, 10);
        assert_eq!(mixer.master.at(0), kept, "{raw}");
    }
}

#[test]
fn the_cap_keeps_the_last_plays_of_a_sound_in_order() {
    let mut audio = AudioCommands::new();
    for _ in 0..3 {
        audio.play(AudioHandle(0));
    }
    audio.stop(AudioHandle(1));
    let b1 = audio.play(AudioHandle(1));
    audio.set_master_volume(0.5);
    let a4 = audio.play(AudioHandle(0));
    let b2 = audio.play(AudioHandle(1));
    let a5 = audio.play_looping(AudioHandle(0));
    let mut commands = audio.drain();
    cap_plays(
        &mut commands,
        |handle| {
            if handle == AudioHandle(0) { 2 } else { 64 }
        },
    );
    let kept: Vec<Option<Voice>> = commands
        .iter()
        .map(|command| play_parts(command).map(|(_, voice, ..)| voice))
        .collect();
    assert_eq!(kept, [None, Some(b1), None, Some(a4), Some(b2), Some(a5)]);
    assert!(matches!(commands[0], AudioCommand::Stop { handle } if handle == AudioHandle(1)));
    assert!(matches!(commands[2], AudioCommand::SetMasterVolume(volume) if volume == 0.5));
}

/// An outbox on a real ring, its latch knowing `limits`, and the ring's
/// consumer.
fn outbox(limits: Vec<Option<u32>>) -> (Outbox, Consumer<AudioCommand>) {
    let (producer, consumer) = RingBuffer::new(RING_CAPACITY);
    (Outbox::new(producer, Latch::new(limits)), consumer)
}

/// Every command in the ring, in order.
fn pop_all(consumer: &mut Consumer<AudioCommand>) -> Vec<AudioCommand> {
    std::iter::from_fn(|| consumer.pop().ok()).collect()
}

/// The master volume each command sets, in order.
fn masters(commands: &[AudioCommand]) -> Vec<f32> {
    commands
        .iter()
        .filter_map(|command| match *command {
            AudioCommand::SetMasterVolume(volume) => Some(volume),
            _ => None,
        })
        .collect()
}

#[test]
fn commands_past_the_ring_wait_in_order_and_flush_moves_them() {
    let (mut outbox, mut consumer) = outbox(Vec::new());
    for index in 0..1_500 {
        outbox.send(AudioCommand::SetMasterVolume(index as f32));
    }
    assert_eq!(consumer.slots(), 1_024);
    assert_eq!(outbox.waiting.len(), 476);
    let mut received = Vec::new();
    for _ in 0..100 {
        received.push(consumer.pop().unwrap());
    }
    // No new command: the flush alone moves the waiting ones.
    outbox.flush();
    assert_eq!(outbox.waiting.len(), 376);
    while outbox.waiting.len() + consumer.slots() > 0 {
        received.extend(pop_all(&mut consumer));
        outbox.flush();
    }
    outbox.send(AudioCommand::SetMasterVolume(-1.0));
    received.extend(pop_all(&mut consumer));
    let mut expected: Vec<f32> = (0..1_500).map(|index| index as f32).collect();
    expected.push(-1.0);
    assert_eq!(masters(&received), expected);
}

#[test]
fn with_4096_waiting_one_shots_drop_and_every_other_command_waits() {
    let (mut outbox, mut consumer) = outbox(vec![Some(64)]);
    for _ in 0..1_024 + 4_096 {
        outbox.send(AudioCommand::SetMasterVolume(0.5));
    }
    assert_eq!(outbox.waiting.len(), 4_096);
    let mut audio = AudioCommands::new();
    audio.play(AudioHandle(0));
    let looped = audio.play_looping(AudioHandle(0));
    audio.stop_all();
    audio.set_master_volume(0.25);
    audio.play(AudioHandle(0));
    for command in audio.drain() {
        outbox.send(command);
    }
    assert_eq!(outbox.dropped_total, 2);
    assert_eq!(outbox.waiting.len(), 4_099);
    let mut received = Vec::new();
    while outbox.waiting.len() + consumer.slots() > 0 {
        received.extend(pop_all(&mut consumer));
        outbox.flush();
    }
    assert_eq!(received.len(), 1_024 + 4_096 + 3);
    let tail = &received[received.len() - 3..];
    assert_eq!(play_parts(&tail[0]).map(|parts| parts.1), Some(looped));
    assert!(matches!(tail[1], AudioCommand::StopAll));
    assert!(matches!(tail[2], AudioCommand::SetMasterVolume(volume) if volume == 0.25));
}

#[test]
fn one_callback_takes_at_most_a_ring_of_commands_and_fills_its_buffer() {
    let mut mixer = mixer_of(vec![sound(1_000, 0.5, 0.5, AudioBus::Effects, 64)], 2);
    let mut audio = AudioCommands::new();
    audio.play_looping(AudioHandle(0));
    apply_all(&mut mixer, &audio.drain());
    let (mut producer, mut consumer) = RingBuffer::new(RING_CAPACITY);
    for _ in 0..RING_CAPACITY {
        producer.push(AudioCommand::SetMasterVolume(1.0)).unwrap();
    }
    // A producer refilling the ring as fast as the callback drains it.
    let stop = Arc::new(AtomicBool::new(false));
    let feeder = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let _ = producer.push(AudioCommand::SetMasterVolume(1.0));
            }
        })
    };
    let mut output = vec![0.0; 512 * 2];
    let taken = mixer.callback(&mut consumer, &mut output);
    stop.store(true, Ordering::Relaxed);
    feeder.join().unwrap();
    assert_eq!(taken, RING_CAPACITY);
    assert!(output.iter().all(|&sample| sample == 0.5));
}

#[test]
fn latch_holds_the_master_each_set_bus_and_the_loops_at_their_last_volume() {
    // Sound 0 takes two voices.
    let mut latch = Latch::new(vec![Some(2), Some(64)]);
    let mut audio = AudioCommands::new();
    audio.set_master_volume(0.5);
    audio.set_bus_volume(AudioBus::Music, 0.25);
    let theme = audio.play_faded(AudioHandle(1), 0.8, true, 2.0);
    let hum = audio.play_looping(AudioHandle(0));
    let gone = audio.play_looping(AudioHandle(1));
    audio.set_voice_volume(theme, 0.3, 1.0);
    audio.stop_voice(gone, 0.5);
    audio.set_master_volume(0.75);
    for command in audio.drain() {
        latch.admit(&command);
    }
    let mut replay = VecDeque::new();
    latch.replay(&mut replay);
    let replay: Vec<AudioCommand> = replay.into();
    assert_eq!(replay.len(), 4);
    assert!(matches!(replay[0], AudioCommand::SetMasterVolume(volume) if volume == 0.75));
    assert!(matches!(
        replay[1],
        AudioCommand::SetBusVolume { bus: AudioBus::Music, volume, .. } if volume == 0.25
    ));
    // Each loop from its start under its own voice, at its last volume.
    assert_eq!(
        play_parts(&replay[2]),
        Some((AudioHandle(1), theme, 0.3, true, 0.0))
    );
    assert_eq!(
        play_parts(&replay[3]),
        Some((AudioHandle(0), hum, 1.0, true, 0.0))
    );
}

#[test]
fn latch_never_holds_a_one_shot_and_drops_stopped_and_limited_loops() {
    let mut latch = Latch::new(vec![Some(2), Some(64), None]);
    let mut audio = AudioCommands::new();
    audio.play(AudioHandle(0));
    audio.play_looping(AudioHandle(0));
    let second = audio.play_looping(AudioHandle(0));
    let other = audio.play_looping(AudioHandle(1));
    // The sound's loops fill its limit: a one-shot's play ends the oldest.
    audio.play(AudioHandle(0));
    // A sound that plays nothing is never latched.
    audio.play_looping(AudioHandle(2));
    for command in audio.drain() {
        latch.admit(&command);
    }
    let held = |latch: &Latch| latch.loops.iter().map(|l| l.voice).collect::<Vec<_>>();
    assert_eq!(held(&latch), [second, other]);
    audio.stop(AudioHandle(1));
    for command in audio.drain() {
        latch.admit(&command);
    }
    assert_eq!(held(&latch), [second]);
    audio.stop_all();
    for command in audio.drain() {
        latch.admit(&command);
    }
    assert!(held(&latch).is_empty());
}

#[test]
fn latch_loops_equal_the_mixers_whether_one_shots_end_before_or_after_the_next_play() {
    // Every sequence of six plays of a two-voice, 10-frame sound; between
    // plays the mixer renders 20 frames, so a one-shot ends first, or 1.
    let mut checked = 0;
    for between in [20, 1] {
        for pattern in 0..64_u32 {
            let mut mixer = mixer_of(vec![sound(10, 0.1, 0.1, AudioBus::Effects, 2)], 2);
            let mut latch = Latch::new(vec![Some(2)]);
            let mut audio = AudioCommands::new();
            for step in 0..6 {
                if pattern & (1 << step) != 0 {
                    audio.play_looping(AudioHandle(0));
                } else {
                    audio.play(AudioHandle(0));
                }
                for command in audio.drain() {
                    latch.admit(&command);
                    mixer.apply(&command);
                }
                render(&mut mixer, between);
                let latched: Vec<Voice> = latch.loops.iter().map(|l| l.voice).collect();
                assert_eq!(
                    latched,
                    playing_loops(&mixer),
                    "pattern {pattern:06b}, step {step}, {between} frames between"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 2 * 64 * 6);
}

#[test]
fn resync_past_65536_waiting_brings_the_mixer_to_the_latch() {
    let (mut outbox, mut consumer) = outbox(vec![Some(2), Some(64)]);
    let mut audio = AudioCommands::new();
    audio.set_master_volume(0.5);
    audio.set_bus_volume(AudioBus::Effects, 0.25);
    let theme = audio.play_looping(AudioHandle(1));
    let hum = audio.play_looping(AudioHandle(0));
    audio.play(AudioHandle(0));
    for command in audio.drain() {
        outbox.send(command);
    }
    // A callback that stopped: the ring fills and the queue grows.
    let mut sent = 0;
    while outbox.waiting.len() < WAITING_RESYNCS {
        outbox.send(AudioCommand::SetMasterVolume(0.5));
        sent += 1;
        if sent == 2_000 {
            audio.set_voice_volume(theme, 0.6, 0.0);
            audio.stop_voice(hum, 0.0);
            audio.play(AudioHandle(1));
            for command in audio.drain() {
                outbox.send(command);
            }
        }
    }
    assert_eq!(outbox.waiting.len(), WAITING_RESYNCS);
    // The command past the bound.
    outbox.send(AudioCommand::SetMasterVolume(0.5));
    let waiting: Vec<AudioCommand> = outbox.waiting.iter().cloned().collect();
    assert_eq!(waiting.len(), 4, "{waiting:?}");
    assert!(matches!(waiting[0], AudioCommand::StopAll));
    assert!(matches!(waiting[1], AudioCommand::SetMasterVolume(volume) if volume == 0.5));
    assert!(matches!(
        waiting[2],
        AudioCommand::SetBusVolume { bus: AudioBus::Effects, volume, .. } if volume == 0.25
    ));
    assert_eq!(
        play_parts(&waiting[3]),
        Some((AudioHandle(1), theme, 0.6, true, 0.0))
    );
    // The one-shot that waited counts as dropped; the ring's are sent.
    assert_eq!(outbox.dropped_total, 1);
    // The callback drains it all; the mixer ends at the latch's state.
    let mut mixer = mixer_of(vec![tone(2), tone(64)], 2);
    let mut output = vec![0.0; 64 * 2];
    while outbox.waiting.len() + consumer.slots() > 0 {
        mixer.callback(&mut consumer, &mut output);
        outbox.flush();
    }
    render(&mut mixer, 20);
    assert_eq!(playing_loops(&mixer), [theme]);
    let playing = mixer
        .voices
        .iter()
        .flatten()
        .find(|voice| voice.voice == theme && !voice.stopping)
        .unwrap();
    assert_eq!(playing.gain.at(0), 0.6);
    assert_eq!(mixer.master.at(0), 0.5);
    assert_eq!(mixer.buses[bus_index(AudioBus::Effects)].at(0), 0.25);
}

#[test]
fn lost_devices_and_invalidated_streams_mark_the_stream_lost_and_other_errors_count() {
    use cpal::ErrorKind;
    let kinds = [
        ErrorKind::DeviceBusy,
        ErrorKind::DeviceChanged,
        ErrorKind::DeviceNotAvailable,
        ErrorKind::HostUnavailable,
        ErrorKind::InvalidInput,
        ErrorKind::PermissionDenied,
        ErrorKind::RealtimeDenied,
        ErrorKind::ResourceExhausted,
        ErrorKind::StreamInvalidated,
        ErrorKind::UnsupportedConfig,
        ErrorKind::UnsupportedOperation,
        ErrorKind::Xrun,
        ErrorKind::BackendError,
        ErrorKind::Other,
    ];
    for kind in kinds {
        let lost = matches!(
            kind,
            ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated
        );
        let health = StreamHealth::default();
        health.report(kind);
        assert_eq!(health.lost.load(Ordering::Acquire), lost, "{kind:?}");
        assert_eq!(
            health.errors.load(Ordering::Relaxed),
            u64::from(!lost),
            "{kind:?}"
        );
    }
    let health = StreamHealth::default();
    for kind in kinds {
        health.report(kind);
    }
    assert!(health.lost.load(Ordering::Acquire));
    assert_eq!(health.errors.load(Ordering::Relaxed), 12);
}

#[test]
fn at_a_lost_stream_the_queue_clears_and_the_latch_keeps_what_waited_less_its_one_shots() {
    let (mut outbox, _consumer) = outbox(vec![Some(64), Some(64)]);
    for _ in 0..RING_CAPACITY {
        outbox.send(AudioCommand::SetMasterVolume(0.5));
    }
    // A loop, a bus volume and a one-shot wait behind the full ring.
    let mut audio = AudioCommands::new();
    let theme = audio.play_looping(AudioHandle(0));
    audio.set_bus_volume(AudioBus::Music, 0.25);
    audio.play(AudioHandle(1));
    for command in audio.drain() {
        outbox.send(command);
    }
    assert_eq!(outbox.waiting.len(), 3);
    outbox.lose();
    assert!(outbox.waiting.is_empty());
    let mut replay = VecDeque::new();
    outbox.latch.replay(&mut replay);
    let replay: Vec<AudioCommand> = replay.into();
    assert_eq!(replay.len(), 3, "{replay:?}");
    assert!(matches!(replay[0], AudioCommand::SetMasterVolume(volume) if volume == 0.5));
    assert!(matches!(
        replay[1],
        AudioCommand::SetBusVolume { bus: AudioBus::Music, volume, .. } if volume == 0.25
    ));
    assert_eq!(
        play_parts(&replay[2]),
        Some((AudioHandle(0), theme, 1.0, true, 0.0))
    );
}

#[test]
fn while_the_stream_is_lost_commands_change_the_latch_and_nothing_waits() {
    let (mut outbox, consumer) = outbox(vec![Some(64)]);
    outbox.lose();
    let mut audio = AudioCommands::new();
    let first = audio.play_looping(AudioHandle(0));
    let second = audio.play_looping(AudioHandle(0));
    audio.stop_voice(first, 0.0);
    audio.set_master_volume(0.3);
    audio.play(AudioHandle(0));
    for command in audio.drain() {
        outbox.send(command);
    }
    outbox.flush();
    assert!(outbox.waiting.is_empty());
    assert_eq!(consumer.slots(), 0, "nothing reaches the lost ring");
    let held: Vec<Voice> = outbox.latch.loops.iter().map(|l| l.voice).collect();
    assert_eq!(held, [second]);
    assert_eq!(outbox.latch.master, Some(0.3));
}

#[test]
fn a_rebuild_sends_the_latch_replay_first_into_the_new_ring() {
    let (mut outbox, _lost_ring) = outbox(vec![Some(64)]);
    let mut audio = AudioCommands::new();
    audio.set_master_volume(0.4);
    let theme = audio.play_faded(AudioHandle(0), 0.7, true, 3.0);
    audio.play(AudioHandle(0));
    for command in audio.drain() {
        outbox.send(command);
    }
    outbox.lose();
    audio.set_bus_volume(AudioBus::Effects, 0.5);
    for command in audio.drain() {
        outbox.send(command);
    }
    let (producer, mut consumer) = RingBuffer::new(RING_CAPACITY);
    outbox.reconnect(producer);
    audio.set_master_volume(0.9);
    for command in audio.drain() {
        outbox.send(command);
    }
    let received = pop_all(&mut consumer);
    assert_eq!(received.len(), 4, "{received:?}");
    assert!(matches!(received[0], AudioCommand::SetMasterVolume(volume) if volume == 0.4));
    assert!(matches!(
        received[1],
        AudioCommand::SetBusVolume { bus: AudioBus::Effects, volume, .. } if volume == 0.5
    ));
    // The loop from its start at its volume, without its fade.
    assert_eq!(
        play_parts(&received[2]),
        Some((AudioHandle(0), theme, 0.7, true, 0.0))
    );
    assert!(matches!(received[3], AudioCommand::SetMasterVolume(volume) if volume == 0.9));
    // A new mixer on the ring comes back to the latch's state.
    let mut mixer = mixer_of(vec![tone(64)], 2);
    apply_all(&mut mixer, &received);
    render(&mut mixer, 20);
    assert_eq!(playing_loops(&mixer), [theme]);
    assert_eq!(mixer.master.at(0), 0.9);
    assert_eq!(mixer.buses[bus_index(AudioBus::Effects)].at(0), 0.5);
}
