//! The audio device (`D-029`, `D-034`): a `cpal` stream whose callback owns
//! a mixer, fed by the main thread's outbox through a ring of commands.
//!
//! The mixer plays up to [`MAX_VOICES`] voices, each at its volume × its
//! bus's volume × the master volume, with linear ramps, and clamps the mix
//! to [-1, 1]. Nothing in the callback allocates. The outbox never drops a
//! command whose state matters: what does not fit the ring waits, in order,
//! and only one-shot plays are dropped once too many wait. Its latch keeps
//! the state the mixer will reach (the master and bus volumes and the
//! loops), so a stalled callback is resynchronized from it, and a lost
//! device is reopened with it.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use tungsten_core::assets::{AudioHandle, SoundData, SoundRegistry};
use tungsten_core::audio::{AudioBus, AudioCommand, MAX_VOICES, Voice};

/// The command ring's capacity (`D-034`), allocated once at init. A
/// callback takes at most this many commands before it renders.
const RING_CAPACITY: usize = 1_024;

/// Waiting commands at which a new one-shot play is dropped and counted.
const WAITING_DROPS_ONE_SHOTS: usize = 4_096;

/// Waiting commands past which the outbox resynchronizes from its latch.
const WAITING_RESYNCS: usize = 65_536;

/// The shortest ramp, in seconds: the master's, a bus's and a stop's, and
/// the least a voice's volume change takes, so no step clicks.
const MIN_RAMP_SECONDS: f32 = 0.010;

/// The longest a count waits before it is logged again.
const LOG_INTERVAL: Duration = Duration::from_secs(1);

/// How long a failed rebuild of a lost stream waits before the next try.
const REBUILD_RETRY: Duration = Duration::from_secs(2);

/// How many buses there are.
const BUSES: usize = AudioBus::ALL.len();

/// The voice slots, as a `usize`.
const VOICES: usize = MAX_VOICES as usize;

/// Audio subsystem: the live stream, whose callback owns the mixer, and the
/// outbox that feeds it. A stream the device lost is dropped and the
/// default device opened again, every two seconds until one opens, and the
/// latch's state replayed into it: the master, the buses and the loops,
/// from their start.
pub struct AudioSystem {
    /// The live stream; `None` while it is lost.
    stream: Option<cpal::Stream>,
    /// What the stream's error callback reports.
    health: Arc<StreamHealth>,
    outbox: Outbox,
    /// When a lost stream's next rebuild may try.
    retry_at: Option<Instant>,
    /// A rebuild has failed since the loss, so the next failure is quiet.
    rebuild_failed: bool,
    last_error_log: Option<Instant>,
}

impl AudioSystem {
    /// Open the default output device and start a stream whose callback owns
    /// a mixer of the registry's sounds, prepared for the device's rate.
    ///
    /// # Errors
    ///
    /// No output device, no output config, or a stream the device refuses.
    pub fn init(sound_registry: &SoundRegistry) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No audio output device found"))?;

        let config = device
            .default_output_config()
            .map_err(|e| anyhow::anyhow!("Failed to get output config: {e}"))?;

        log::info!(
            "Audio device: '{}', format: {:?}, sample rate: {}, channels: {}",
            device
                .description()
                .map_or_else(|_| "unknown".to_string(), |d| d.name().to_string()),
            config.sample_format(),
            config.sample_rate(),
            config.channels(),
        );

        let sounds = prepare_sounds(sound_registry, config.sample_rate());
        let latch = Latch::new(sound_limits(&sounds));
        let mut mixer = Mixer::new(sounds, config.sample_rate(), usize::from(config.channels()));
        let (producer, mut consumer) = RingBuffer::<AudioCommand>::new(RING_CAPACITY);
        let health = Arc::new(StreamHealth::default());
        let reporter = Arc::clone(&health);

        let stream = device
            .build_output_stream(
                config.into(),
                move |output: &mut [f32], _info| {
                    mixer.callback(&mut consumer, output);
                },
                // The audio thread only sets flags; the stage logs.
                move |err: cpal::Error| reporter.report(err.kind()),
                None,
            )
            .map_err(|e| anyhow::anyhow!("Failed to build output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| anyhow::anyhow!("Failed to start audio stream: {e}"))?;

        Ok(AudioSystem {
            stream: Some(stream),
            health,
            outbox: Outbox::new(producer, latch),
            retry_at: None,
            rebuild_failed: false,
            last_error_log: None,
        })
    }

    /// Send a command to the mixer. Waiting commands go first, then this one
    /// goes into the ring if it has room, or waits behind them in order.
    /// While 4,096 or more wait, a new one-shot play is dropped and counted;
    /// every other command waits, so no state is lost. Past 65,536 waiting,
    /// the queue is replaced by `StopAll` and the state the commands so far
    /// lead to: the master and bus volumes and the loops, from their start.
    pub fn send(&mut self, cmd: AudioCommand) {
        self.outbox.send(cmd);
    }

    /// Move waiting commands into the ring as far as it has room; never
    /// blocks. The app calls it every frame, so waiting commands move as
    /// soon as the callback frees room; a caller sending outside the app
    /// calls it too.
    pub fn flush(&mut self) {
        self.outbox.flush();
    }

    /// The stage's check, every frame: log the stream's error count at most
    /// once a second; on a lost stream, log once, clear the waiting queue
    /// and drop the stream, so one callback thread runs at a time; then,
    /// when a try is due, open the default device again and replay the
    /// latch into the new ring. A failed rebuild tries again in two seconds.
    pub(crate) fn maintain(&mut self, sounds: &SoundRegistry) {
        self.log_errors();
        if self.stream.is_some() && self.health.lost.load(Ordering::Acquire) {
            log::warn!("Audio output lost; reopening the default device");
            self.outbox.lose();
            self.stream = None;
            self.retry_at = Some(Instant::now());
            self.rebuild_failed = false;
        }
        if self.stream.is_some() || self.retry_at.is_none_or(|at| Instant::now() < at) {
            return;
        }
        match AudioSystem::init(sounds) {
            Ok(fresh) => {
                log::info!("Audio output reopened");
                let AudioSystem {
                    stream,
                    health,
                    outbox,
                    ..
                } = fresh;
                self.stream = stream;
                self.health = health;
                self.outbox.reconnect(outbox.producer);
                self.retry_at = None;
            }
            Err(error) => {
                if !self.rebuild_failed {
                    log::warn!("Audio output not reopened ({error}); trying every 2 s");
                    self.rebuild_failed = true;
                }
                self.retry_at = Some(Instant::now() + REBUILD_RETRY);
            }
        }
    }

    /// Log the errors the stream counted, at most once a second.
    fn log_errors(&mut self) {
        let now = Instant::now();
        if self
            .last_error_log
            .is_some_and(|last| now.duration_since(last) < LOG_INTERVAL)
        {
            return;
        }
        let errors = self.health.errors.swap(0, Ordering::Relaxed);
        if errors > 0 {
            log::warn!(
                "Audio stream: {errors} errors (underruns among them) since the last report"
            );
            self.last_error_log = Some(now);
        }
    }
}

/// What a stream error means: the stream is gone, or a glitch to count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamFault {
    /// The device went or the stream must be rebuilt.
    Lost,
    /// Anything else, an underrun among them: counted and logged.
    Counted,
}

impl StreamFault {
    fn of(kind: cpal::ErrorKind) -> Self {
        match kind {
            cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => Self::Lost,
            _ => Self::Counted,
        }
    }
}

/// The flags a stream's error callback sets for the stage: lost, and how
/// many other errors came.
#[derive(Debug, Default)]
struct StreamHealth {
    lost: AtomicBool,
    errors: AtomicU64,
}

impl StreamHealth {
    fn report(&self, kind: cpal::ErrorKind) {
        match StreamFault::of(kind) {
            StreamFault::Lost => self.lost.store(true, Ordering::Release),
            StreamFault::Counted => {
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// The plays of `commands` past each sound's limit in one frame dropped,
/// keeping the last ones: the earlier voices never start. Every other
/// command stays in order.
pub(crate) fn cap_plays(commands: &mut Vec<AudioCommand>, limit: impl Fn(AudioHandle) -> u32) {
    if !commands
        .iter()
        .any(|command| matches!(command, AudioCommand::Play { .. }))
    {
        return;
    }
    let mut kept: HashMap<AudioHandle, u32> = HashMap::new();
    let mut keep = vec![true; commands.len()];
    for (index, command) in commands.iter().enumerate().rev() {
        if let AudioCommand::Play { handle, .. } = *command {
            let count = kept.entry(handle).or_insert(0);
            if *count >= limit(handle) {
                keep[index] = false;
            } else {
                *count += 1;
            }
        }
    }
    let mut index = 0;
    commands.retain(|_| {
        index += 1;
        keep[index - 1]
    });
}

/// A sound as the mixer plays it: stereo PCM at the device's rate, its bus
/// and its voice limit.
#[derive(Debug, Clone)]
struct MixSound {
    /// Interleaved left and right samples; never empty.
    pcm: Vec<f32>,
    /// Its bus's index in `AudioBus::ALL`.
    bus: usize,
    limit: u32,
}

/// The registry's sounds by handle, prepared for `rate`; a sound with no
/// samples plays nothing, so it has none.
fn prepare_sounds(registry: &SoundRegistry, rate: u32) -> Vec<Option<MixSound>> {
    let mut sounds = Vec::new();
    for (handle, data) in registry.iter() {
        let index = handle.0 as usize;
        if sounds.len() <= index {
            sounds.resize(index + 1, None);
        }
        let pcm = prepare_pcm(data, rate);
        if !pcm.is_empty() {
            sounds[index] = Some(MixSound {
                pcm,
                bus: bus_index(registry.get_bus(handle)),
                limit: registry.get_max_voices(handle).clamp(1, MAX_VOICES),
            });
        }
    }
    sounds
}

/// Each sound's voice limit, or `None` for a sound that plays nothing: what
/// the latch needs of the mixer's table.
fn sound_limits(sounds: &[Option<MixSound>]) -> Vec<Option<u32>> {
    sounds
        .iter()
        .map(|sound| sound.as_ref().map(|sound| sound.limit))
        .collect()
}

/// A bus's place in `AudioBus::ALL`.
fn bus_index(bus: AudioBus) -> usize {
    AudioBus::ALL
        .iter()
        .position(|&each| each == bus)
        .unwrap_or(0)
}

/// Resample to `target_rate` and lay out as interleaved stereo: a mono
/// source on both sides, a wider one by its first two channels.
fn prepare_pcm(data: &SoundData, target_rate: u32) -> Vec<f32> {
    let src_channels = usize::from(data.channels.max(1));
    let stereo: Vec<f32> = data
        .samples
        .chunks_exact(src_channels)
        .flat_map(|frame| {
            if src_channels == 1 {
                [frame[0], frame[0]]
            } else {
                [frame[0], frame[1]]
            }
        })
        .collect();

    let src_frames = stereo.len() / 2;
    if data.sample_rate == target_rate || src_frames == 0 {
        return stereo;
    }
    let ratio = f64::from(data.sample_rate) / f64::from(target_rate);
    let target_frames = (src_frames as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(target_frames * 2);
    for i in 0..target_frames {
        let src_pos = i as f64 * ratio;
        let idx0 = (src_pos as usize).min(src_frames - 1);
        let idx1 = (idx0 + 1).min(src_frames - 1);
        let frac = (src_pos - idx0 as f64) as f32;
        out.push(lerp(stereo[idx0 * 2], stereo[idx1 * 2], frac));
        out.push(lerp(stereo[idx0 * 2 + 1], stereo[idx1 * 2 + 1], frac));
    }
    out
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A command's volume as a gain: one that is not finite or is negative is 0,
/// since a command's fields can be written outside `AudioCommands`' checks
/// and a NaN gain turns every sample into NaN.
fn playable_volume(volume: f32) -> f32 {
    if volume.is_finite() && volume > 0.0 {
        volume
    } else {
        0.0
    }
}

/// A command's fade in seconds: one that is not finite or is negative is 0.
fn playable_seconds(seconds: f32) -> f32 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
    }
}

/// A gain moving linearly from `from` to `to` over `frames` frames, of which
/// `elapsed` have played.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ramp {
    from: f32,
    to: f32,
    frames: u32,
    elapsed: u32,
}

impl Ramp {
    /// A gain at rest at `value`.
    fn at_rest(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            frames: 0,
            elapsed: 0,
        }
    }

    /// The gain `offset` frames into the next buffer.
    fn at(&self, offset: usize) -> f32 {
        let elapsed = self.elapsed as usize + offset;
        if elapsed >= self.frames as usize {
            self.to
        } else {
            self.from + (self.to - self.from) * (elapsed as f32 / self.frames as f32)
        }
    }

    /// The ramp ends at or before `offset` frames into the next buffer.
    fn done_at(&self, offset: usize) -> bool {
        self.elapsed as usize + offset >= self.frames as usize
    }

    /// Head for `to` over `frames`, from where the gain is now.
    fn retarget(&mut self, to: f32, frames: u32) {
        *self = Self {
            from: self.at(0),
            to,
            frames,
            elapsed: 0,
        };
    }

    fn advance(&mut self, frames: usize) {
        self.elapsed = (self.elapsed as usize + frames).min(self.frames as usize) as u32;
    }
}

/// One voice in a mixer slot.
#[derive(Debug, Clone, Copy)]
struct PlayingVoice {
    voice: Voice,
    handle: AudioHandle,
    /// Its sound's bus, by index.
    bus: usize,
    /// The next frame to play.
    cursor: usize,
    looping: bool,
    /// Start order: the lower, the older.
    started: u64,
    gain: Ramp,
    /// Fading to 0 after a stop: it counts toward no limit, takes no
    /// command, and frees its slot when the ramp ends.
    stopping: bool,
}

impl PlayingVoice {
    /// Playing, not fading after a stop.
    fn playing(&self) -> bool {
        !self.stopping
    }
}

/// The callback's mixer: the sounds, [`MAX_VOICES`] voice slots, the buses'
/// and the master's gains. Built on the main thread and moved into the
/// callback; nothing in it allocates once built.
pub(crate) struct Mixer {
    sounds: Vec<Option<MixSound>>,
    /// Exactly `MAX_VOICES` slots.
    voices: Vec<Option<PlayingVoice>>,
    buses: [Ramp; BUSES],
    master: Ramp,
    channels: usize,
    sample_rate: u32,
    /// `MIN_RAMP_SECONDS` in frames.
    min_ramp: u32,
    next_start: u64,
}

impl Mixer {
    fn new(sounds: Vec<Option<MixSound>>, sample_rate: u32, channels: usize) -> Self {
        let mut mixer = Self {
            sounds,
            voices: vec![None; VOICES],
            buses: [Ramp::at_rest(1.0); BUSES],
            master: Ramp::at_rest(1.0),
            channels: channels.max(1),
            sample_rate,
            min_ramp: 0,
            next_start: 0,
        };
        mixer.min_ramp = mixer.frames(MIN_RAMP_SECONDS).max(1);
        mixer
    }

    /// `seconds` in frames at the device's rate.
    fn frames(&self, seconds: f32) -> u32 {
        (playable_seconds(seconds) * self.sample_rate as f32).round() as u32
    }

    /// The callback: takes at most one ring's worth of commands, so a
    /// producer refilling the ring cannot hold the output back, then renders
    /// `output`. Returns the commands it took.
    fn callback(&mut self, commands: &mut Consumer<AudioCommand>, output: &mut [f32]) -> usize {
        let mut taken = 0;
        while taken < RING_CAPACITY {
            let Ok(command) = commands.pop() else {
                break;
            };
            self.apply(&command);
            taken += 1;
        }
        self.render(output);
        taken
    }

    fn apply(&mut self, command: &AudioCommand) {
        match *command {
            AudioCommand::Play {
                handle,
                voice,
                volume,
                looping,
                fade,
                ..
            } => self.play(handle, voice, volume, looping, fade),
            AudioCommand::Stop { handle } => {
                self.stop_where(|playing| playing.handle == handle, self.min_ramp);
            }
            AudioCommand::StopAll => self.stop_where(|_| true, self.min_ramp),
            AudioCommand::SetMasterVolume(volume) => {
                self.master
                    .retarget(playable_volume(volume).min(1.0), self.min_ramp);
            }
            AudioCommand::StopVoice { voice, fade, .. } => {
                let frames = self.frames(fade).max(self.min_ramp);
                self.stop_where(|playing| playing.voice == voice, frames);
            }
            AudioCommand::SetVoiceVolume {
                voice,
                volume,
                fade,
                ..
            } => {
                let frames = self.frames(fade).max(self.min_ramp);
                for playing in self.voices.iter_mut().flatten() {
                    if playing.playing() && playing.voice == voice {
                        playing.gain.retarget(playable_volume(volume), frames);
                    }
                }
            }
            AudioCommand::SetBusVolume { bus, volume, .. } => {
                self.buses[bus_index(bus)]
                    .retarget(playable_volume(volume).min(1.0), self.min_ramp);
            }
            _ => {}
        }
    }

    /// Start `voice`. At its sound's limit, the sound's oldest one-shot ends
    /// at once, or its oldest loop when none of its one-shots plays. Then it
    /// takes a free slot, else a stopping voice's, else the oldest one-shot's
    /// of any sound; with every slot holding a playing loop it is dropped.
    fn play(&mut self, handle: AudioHandle, voice: Voice, volume: f32, looping: bool, fade: f32) {
        let Some(sound) = self.sounds.get(handle.0 as usize).and_then(Option::as_ref) else {
            return;
        };
        let (bus, limit) = (sound.bus, sound.limit);
        let of_sound = |playing: &PlayingVoice| playing.playing() && playing.handle == handle;
        let playing = self.voices.iter().flatten().filter(|v| of_sound(v)).count();
        if playing >= limit as usize {
            let victim = self
                .oldest(|v| of_sound(v) && !v.looping)
                .or_else(|| self.oldest(|v| of_sound(v) && v.looping));
            if let Some(slot) = victim {
                self.voices[slot] = None;
            }
        }
        let slot = self
            .voices
            .iter()
            .position(Option::is_none)
            .or_else(|| self.oldest(|v| v.stopping))
            .or_else(|| self.oldest(|v| v.playing() && !v.looping));
        let Some(slot) = slot else {
            return;
        };
        let volume = playable_volume(volume);
        let gain = Ramp {
            from: 0.0,
            to: volume,
            frames: self.frames(fade),
            elapsed: 0,
        };
        let gain = if gain.frames == 0 {
            Ramp::at_rest(volume)
        } else {
            gain
        };
        self.voices[slot] = Some(PlayingVoice {
            voice,
            handle,
            bus,
            cursor: 0,
            looping,
            started: self.next_start,
            gain,
            stopping: false,
        });
        self.next_start += 1;
    }

    /// The slot of the oldest voice `pick` takes.
    fn oldest(&self, pick: impl Fn(&PlayingVoice) -> bool) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(slot, voice)| voice.as_ref().map(|voice| (slot, voice)))
            .filter(|(_, voice)| pick(voice))
            .min_by_key(|(_, voice)| voice.started)
            .map(|(slot, _)| slot)
    }

    /// Stop every playing voice `pick` takes, ramping it to 0 over `frames`.
    fn stop_where(&mut self, pick: impl Fn(&PlayingVoice) -> bool, frames: u32) {
        for playing in self.voices.iter_mut().flatten() {
            if playing.playing() && pick(playing) {
                playing.stopping = true;
                playing.gain.retarget(0.0, frames);
            }
        }
    }

    /// Mix every voice into `output`, interleaved by the device's channels:
    /// left and right into the first two, the rest silent, or their mean on
    /// a mono device. The mix is clamped to [-1, 1].
    fn render(&mut self, output: &mut [f32]) {
        output.fill(0.0);
        let Mixer {
            sounds,
            voices,
            buses,
            master,
            channels,
            ..
        } = self;
        let channels = *channels;
        let frames = output.len() / channels;
        for slot in voices.iter_mut() {
            let Some(voice) = slot else {
                continue;
            };
            let Some(sound) = sounds.get(voice.handle.0 as usize).and_then(Option::as_ref) else {
                *slot = None;
                continue;
            };
            let length = sound.pcm.len() / 2;
            let bus = buses[voice.bus];
            let mut ended = false;
            for frame in 0..frames {
                if voice.stopping && voice.gain.done_at(frame) {
                    ended = true;
                    break;
                }
                if voice.cursor >= length {
                    if voice.looping {
                        voice.cursor = 0;
                    } else {
                        ended = true;
                        break;
                    }
                }
                let gain = voice.gain.at(frame) * bus.at(frame) * master.at(frame);
                let left = sound.pcm[voice.cursor * 2] * gain;
                let right = sound.pcm[voice.cursor * 2 + 1] * gain;
                let out = &mut output[frame * channels..];
                if channels == 1 {
                    out[0] += f32::midpoint(left, right);
                } else {
                    out[0] += left;
                    out[1] += right;
                }
                voice.cursor += 1;
            }
            voice.gain.advance(frames);
            if ended {
                *slot = None;
            }
        }
        for bus in buses.iter_mut() {
            bus.advance(frames);
        }
        master.advance(frames);
        for sample in output.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

/// A loop the latch holds: the play that started it, at its last volume
/// and with no fade.
#[derive(Debug, Clone)]
struct LatchedLoop {
    voice: Voice,
    handle: AudioHandle,
    play: AudioCommand,
}

/// The state the mixer reaches once it has taken every admitted command: the
/// master volume, each set bus's volume and the loops playing, in start
/// order. It applies the mixer's rules to the commands in admission order,
/// and a one-shot is never latched: under the mixer's limits no one-shot's
/// end decides a loop's fate, so the loops are exactly the mixer's.
#[derive(Debug, Clone)]
pub(crate) struct Latch {
    /// By handle: the sound's voice limit, or `None` when it plays nothing.
    limits: Vec<Option<u32>>,
    master: Option<f32>,
    /// The last `SetBusVolume` each bus took.
    buses: [Option<AudioCommand>; BUSES],
    loops: Vec<LatchedLoop>,
}

impl Latch {
    fn new(limits: Vec<Option<u32>>) -> Self {
        Self {
            limits,
            master: None,
            buses: std::array::from_fn(|_| None),
            loops: Vec::new(),
        }
    }

    /// Apply an admitted command.
    fn admit(&mut self, command: &AudioCommand) {
        match *command {
            AudioCommand::Play {
                handle,
                voice,
                volume,
                looping,
                ..
            } => {
                let Some(limit) = self.limits.get(handle.0 as usize).copied().flatten() else {
                    return;
                };
                // The sound's loops fill its limit only when none of its
                // one-shots plays: then the mixer ends its oldest loop.
                let latched = self.loops.iter().filter(|l| l.handle == handle).count();
                if latched >= limit as usize
                    && let Some(oldest) = self.loops.iter().position(|l| l.handle == handle)
                {
                    self.loops.remove(oldest);
                }
                // With every slot holding a playing loop, a play is dropped.
                if looping && self.loops.len() < VOICES {
                    let mut play = command.clone();
                    if let AudioCommand::Play {
                        volume: last, fade, ..
                    } = &mut play
                    {
                        *last = playable_volume(volume);
                        *fade = 0.0;
                    }
                    self.loops.push(LatchedLoop {
                        voice,
                        handle,
                        play,
                    });
                }
            }
            AudioCommand::Stop { handle } => self.loops.retain(|l| l.handle != handle),
            AudioCommand::StopAll => self.loops.clear(),
            AudioCommand::SetMasterVolume(volume) => self.master = Some(volume),
            AudioCommand::StopVoice { voice, .. } => self.loops.retain(|l| l.voice != voice),
            AudioCommand::SetVoiceVolume { voice, volume, .. } => {
                for latched in self.loops.iter_mut().filter(|l| l.voice == voice) {
                    if let AudioCommand::Play { volume: last, .. } = &mut latched.play {
                        *last = playable_volume(volume);
                    }
                }
            }
            AudioCommand::SetBusVolume { bus, .. } => {
                self.buses[bus_index(bus)] = Some(command.clone());
            }
            _ => {}
        }
    }

    /// The commands that bring a mixer to this state: the master volume,
    /// each set bus's volume, then every loop from its start under its own
    /// voice.
    fn replay(&self, out: &mut VecDeque<AudioCommand>) {
        if let Some(master) = self.master {
            out.push_back(AudioCommand::SetMasterVolume(master));
        }
        out.extend(self.buses.iter().flatten().cloned());
        out.extend(self.loops.iter().map(|latched| latched.play.clone()));
    }
}

/// A one-shot play: what the outbox drops when too many commands wait.
fn is_one_shot(command: &AudioCommand) -> bool {
    matches!(command, AudioCommand::Play { looping: false, .. })
}

/// The ring's producer, the commands waiting for room in it, and the latch
/// of what it admitted.
pub(crate) struct Outbox {
    producer: Producer<AudioCommand>,
    waiting: VecDeque<AudioCommand>,
    latch: Latch,
    /// The stream is lost: commands change the latch and nothing waits.
    lost: bool,
    /// One-shot plays dropped since the last log.
    dropped: u64,
    /// Resynchronizations since the last log.
    resyncs: u64,
    last_log: Option<Instant>,
    #[cfg(test)]
    dropped_total: u64,
}

impl Outbox {
    fn new(producer: Producer<AudioCommand>, latch: Latch) -> Self {
        Self {
            producer,
            waiting: VecDeque::new(),
            latch,
            lost: false,
            dropped: 0,
            resyncs: 0,
            last_log: None,
            #[cfg(test)]
            dropped_total: 0,
        }
    }

    fn send(&mut self, command: AudioCommand) {
        if self.lost {
            self.latch.admit(&command);
            return;
        }
        self.flush();
        if is_one_shot(&command) && self.waiting.len() >= WAITING_DROPS_ONE_SHOTS {
            self.count_dropped(1);
            return;
        }
        self.latch.admit(&command);
        if self.waiting.is_empty() {
            match self.producer.push(command) {
                Ok(()) => return,
                Err(PushError::Full(command)) => self.waiting.push_back(command),
            }
        } else {
            self.waiting.push_back(command);
        }
        if self.waiting.len() > WAITING_RESYNCS {
            self.resync();
        }
    }

    fn flush(&mut self) {
        if self.lost {
            return;
        }
        while let Some(command) = self.waiting.pop_front() {
            if let Err(PushError::Full(command)) = self.producer.push(command) {
                self.waiting.push_front(command);
                break;
            }
        }
        self.log_counts();
    }

    /// The stream is lost: what waited is in the latch already, and its
    /// one-shots are stale, so the queue goes.
    fn lose(&mut self) {
        self.lost = true;
        self.waiting.clear();
    }

    /// A new stream's ring: the latch's state goes into it first.
    fn reconnect(&mut self, producer: Producer<AudioCommand>) {
        self.producer = producer;
        self.lost = false;
        self.waiting.clear();
        self.latch.replay(&mut self.waiting);
        self.flush();
    }

    /// Replace the waiting commands with `StopAll` and the latch's state:
    /// the mixer comes back to it once it takes them. The one-shots that
    /// waited are dropped.
    fn resync(&mut self) {
        let one_shots = self.waiting.iter().filter(|c| is_one_shot(c)).count();
        self.count_dropped(one_shots as u64);
        self.resyncs += 1;
        self.waiting.clear();
        self.waiting.push_back(AudioCommand::StopAll);
        self.latch.replay(&mut self.waiting);
    }

    fn count_dropped(&mut self, count: u64) {
        self.dropped += count;
        #[cfg(test)]
        {
            self.dropped_total += count;
        }
    }

    /// Log the counts at most once a second.
    fn log_counts(&mut self) {
        if self.dropped == 0 && self.resyncs == 0 {
            return;
        }
        let now = Instant::now();
        if self
            .last_log
            .is_some_and(|last| now.duration_since(last) < LOG_INTERVAL)
        {
            return;
        }
        log::warn!(
            "Audio: {} one-shot plays dropped and {} resynchronizations while commands waited for the mixer",
            self.dropped,
            self.resyncs
        );
        self.dropped = 0;
        self.resyncs = 0;
        self.last_log = Some(now);
    }
}

#[cfg(test)]
#[path = "tests/audio.rs"]
mod tests;
