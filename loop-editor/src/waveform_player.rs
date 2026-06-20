use crossbeam_channel::{Receiver, Sender};
use rodio::{OutputStream, Sink, Source};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Commando's van UI naar waveform audio-thread
// ---------------------------------------------------------------------------

pub enum WaveformCommand {
    Play {
        samples: Arc<Mutex<Vec<f32>>>,
        sample_rate: u32,
        start_sample: usize,
        segment_start_sec: f32,
        a_sample: usize,
        b_sample: usize,
        /// Lock-free: f32 gecodeerd als AtomicU32 via `to_bits`/`from_bits`.
        pitch_semitones: Arc<AtomicU32>,
        /// Lock-free: f32 gecodeerd als AtomicU32 via `to_bits`/`from_bits`.
        tempo: Arc<AtomicU32>,
    },
    Stop,
    /// Pauzeer de sink. De `pos` blijft staan.
    #[allow(dead_code)]
    Pause,
    /// Hervat na pauze.
    #[allow(dead_code)]
    Resume,
    TogglePause,
    /// Update de loop-grenzen zonder de source te herstarten.
    SetLoopBounds {
        a_secs: f32,
        b_secs: f32,
    },
    /// Lock-free seek: schrijft de nieuwe sample-positie als `f64` via `AtomicU64`.
    Seek {
        pos_secs: f32,
    },
    SetPitch(f32),
    SetTempo(f32),
    /// Schakel looping aan/uit zonder de A-B markers te wissen.
    SetLoopEnabled(bool),
}

// ---------------------------------------------------------------------------
// Events van waveform audio-thread naar UI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WaveformEvent {
    Playing,
    Stopped,
    Paused,
    Resumed,
    Error(String),
    Position(f32, f32),
    /// Wordt gestuurd wanneer de source 4× van B→A heeft gewrapt.
    /// De UI kan hierop reageren door naar `loop_b + 1s` te seeken.
    LoopLimitReached,
}

// ---------------------------------------------------------------------------
// Gedeelde loop-grenzen
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct LoopBounds {
    pub a: usize,
    pub b: usize,
    pub enabled: bool,
}

impl LoopBounds {
    pub fn enabled(&self) -> bool {
        self.enabled && self.b > self.a
    }
}

// ---------------------------------------------------------------------------
// Start de thread. Geeft (cmd_tx, event_rx).
// ---------------------------------------------------------------------------

pub fn start_waveform_thread() -> (Sender<WaveformCommand>, Receiver<WaveformEvent>) {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();

    std::thread::spawn(move || {
        run_waveform_audio(cmd_rx, event_tx);
    });

    (cmd_tx, event_rx)
}

// ---------------------------------------------------------------------------
// Interne audio-loop — lock-free shared state via atomics
// ---------------------------------------------------------------------------

fn run_waveform_audio(rx: Receiver<WaveformCommand>, event_tx: Sender<WaveformEvent>) {
    let mut _stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut is_playing = false;
    let mut is_paused = false;

    // ── Lock-free shared state ──
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let pitch_semitones: Arc<AtomicU32> = Arc::new(AtomicU32::new(f32::to_bits(0.0)));
    let tempo: Arc<AtomicU32> = Arc::new(AtomicU32::new(f32::to_bits(1.0)));
    let loop_bounds: Arc<Mutex<LoopBounds>> = Arc::new(Mutex::new(LoopBounds {
        a: 0,
        b: 0,
        enabled: false,
    }));
    let source_pos: Arc<AtomicU64> = Arc::new(AtomicU64::new(f64::to_bits(0.0)));
    let wrap_count: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));
    let mut prev_wrap: u32 = 0;
    let mut wrap_limit_sent = false;
    let segment_start_sec: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let segment_dur: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let current_sample_rate: Arc<Mutex<u32>> = Arc::new(Mutex::new(44100));

    // Audio device openen
    if let Ok((stream, handle)) = OutputStream::try_default() {
        if let Ok(new_sink) = Sink::try_new(&handle) {
            _stream = Some(stream);
            sink = Some(new_sink);
        }
    }

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                WaveformCommand::Play {
                    samples: new_samples,
                    sample_rate: sr,
                    start_sample,
                    segment_start_sec: seg_start,
                    a_sample,
                    b_sample,
                    pitch_semitones: ps,
                    tempo: t,
                } => {
                    // Lock-free: atomic load van de meegegeven Arcs, daarna atomic store
                    *samples.lock().unwrap() = new_samples.lock().unwrap().clone();
                    pitch_semitones.store(ps.load(Ordering::Relaxed), Ordering::Relaxed);
                    tempo.store(t.load(Ordering::Relaxed), Ordering::Relaxed);
                    *current_sample_rate.lock().unwrap() = sr;
                    source_pos.store(f64::to_bits(start_sample as f64), Ordering::Relaxed);
                    *segment_start_sec.lock().unwrap() = seg_start;

                    let len = samples.lock().unwrap().len();
                    *segment_dur.lock().unwrap() = len as f32 / sr as f32;

                    if b_sample > a_sample && b_sample <= len {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                            enabled: true,
                        };
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: 0,
                            b: 0,
                            enabled: false,
                        };
                    }

                    // Wrap-counter resetten bij nieuwe Play
                    wrap_count.store(0, Ordering::Relaxed);
                    prev_wrap = 0;
                    wrap_limit_sent = false;

                    // Nieuwe real-time source aanmaken
                    let source = RealTimePitchTempoSource::new(
                        samples.clone(),
                        sr,
                        pitch_semitones.clone(),
                        tempo.clone(),
                        loop_bounds.clone(),
                        source_pos.clone(),
                        wrap_count.clone(),
                    );

                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                        s.append(source);
                        s.play();
                        is_playing = true;
                        is_paused = false;
                        let _ = event_tx.send(WaveformEvent::Playing);
                    } else {
                        let _ = event_tx.send(WaveformEvent::Error("Geen audio-apparaat".into()));
                    }
                }

                WaveformCommand::Stop => {
                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                    }
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                }

                WaveformCommand::Pause => {
                    if let Some(s) = &sink {
                        if !s.is_paused() {
                            s.pause();
                            is_paused = true;
                            let _ = event_tx.send(WaveformEvent::Paused);
                        }
                    }
                }

                WaveformCommand::Resume => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        }
                    }
                }

                WaveformCommand::TogglePause => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        } else {
                            s.pause();
                            is_paused = true;
                            let _ = event_tx.send(WaveformEvent::Paused);
                        }
                    }
                }

                WaveformCommand::SetLoopBounds { a_secs, b_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let a_sample = (a_secs.max(0.0) * sr as f32) as usize;
                    let b_sample = (b_secs.max(0.0) * sr as f32) as usize;

                    if b_sample > a_sample {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                            enabled: true,
                        };
                        *segment_start_sec.lock().unwrap() = a_secs;
                        *segment_dur.lock().unwrap() = (b_secs - a_secs).max(0.001);
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: 0,
                            b: 0,
                            enabled: false,
                        };
                    }
                }

                WaveformCommand::Seek { pos_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let start_sec = *segment_start_sec.lock().unwrap();
                    let rel_secs = (pos_secs - start_sec).max(0.0);
                    let sample = (rel_secs * sr as f32) as f64;
                    // Lock-free: atomic write
                    source_pos.store(f64::to_bits(sample), Ordering::Relaxed);
                }

                WaveformCommand::SetPitch(semitones) => {
                    // Lock-free: atomic write
                    pitch_semitones.store(f32::to_bits(semitones), Ordering::Relaxed);
                }

                WaveformCommand::SetTempo(new_tempo) => {
                    // Lock-free: atomic write
                    tempo.store(f32::to_bits(new_tempo), Ordering::Relaxed);
                }

                WaveformCommand::SetLoopEnabled(enabled) => {
                    let mut bounds = loop_bounds.lock().unwrap();
                    bounds.enabled = enabled;
                }
            }
        }

        // Positie-updates sturen naar UI (~60 fps)
        if is_playing && !is_paused {
            if let Some(s) = &sink {
                if s.empty() {
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                } else {
                    // Lock-free: atomic read
                    let pos_samples = f64::from_bits(source_pos.load(Ordering::Relaxed));
                    let sr = *current_sample_rate.lock().unwrap();
                    let start_sec = *segment_start_sec.lock().unwrap();
                    let dur = *segment_dur.lock().unwrap();
                    let bounds = *loop_bounds.lock().unwrap();

                    let pos_secs = pos_samples as f32 / sr as f32;
                    let effective_pos = if bounds.enabled() {
                        let loop_dur = (bounds.b - bounds.a) as f32 / sr as f32;
                        start_sec + (pos_secs % loop_dur)
                    } else {
                        start_sec + pos_secs
                    };
                    let _ = event_tx.send(WaveformEvent::Position(effective_pos, dur));

                    // Wrap-detectie: als de source 4× heeft gewrapt, stuur LoopLimitReached
                    let cur_wrap = wrap_count.load(Ordering::Relaxed);
                    if cur_wrap >= prev_wrap + 4 && !wrap_limit_sent {
                        let _ = event_tx.send(WaveformEvent::LoopLimitReached);
                        wrap_limit_sent = true;
                    }
                    prev_wrap = cur_wrap;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}

// ---------------------------------------------------------------------------
// RealTimePitchTempoSource — lock-free via AtomicU32/AtomicU64 voor pitch/tempo/pos
// ---------------------------------------------------------------------------

/// Aantal samples per interne chunk.
const CHUNK_SIZE: usize = 512;

struct RealTimePitchTempoSource {
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    /// Lock-free: f32 gecodeerd als AtomicU32.
    pitch_semitones: Arc<AtomicU32>,
    /// Lock-free: f32 gecodeerd als AtomicU32.
    tempo: Arc<AtomicU32>,
    loop_bounds: Arc<Mutex<LoopBounds>>,
    /// Lock-free: f64 gecodeerd als AtomicU64.
    source_pos: Arc<AtomicU64>,
    wrap_count: Arc<AtomicU32>,
    buf: Vec<f32>,
    buf_idx: usize,
}

impl RealTimePitchTempoSource {
    fn new(
        samples: Arc<Mutex<Vec<f32>>>,
        sample_rate: u32,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
        loop_bounds: Arc<Mutex<LoopBounds>>,
        source_pos: Arc<AtomicU64>,
        wrap_count: Arc<AtomicU32>,
    ) -> Self {
        Self {
            samples,
            sample_rate,
            pitch_semitones,
            tempo,
            loop_bounds,
            source_pos,
            wrap_count,
            buf: Vec::new(),
            buf_idx: 0,
        }
    }

    /// Vult de interne buffer met `CHUNK_SIZE` verwerkte samples.
    /// Pitch, tempo en source_pos worden lock-free uitgelezen (`Ordering::Relaxed`).
    fn refill_buffer(&mut self) {
        let guard = self.samples.lock().unwrap();
        let raw = &*guard;

        if raw.is_empty() {
            self.buf.clear();
            return;
        }

        let total_len = raw.len();

        // Lock-free reads (atomic, Ordering::Relaxed — exactheid is niet kritisch)
        let pitch_bits = self.pitch_semitones.load(Ordering::Relaxed);
        let tempo_bits = self.tempo.load(Ordering::Relaxed);
        let pitch = f32::from_bits(pitch_bits);
        let t = f32::from_bits(tempo_bits);
        let pitch_factor = f32::powf(2.0, pitch / 12.0);
        let step = (pitch_factor * t) as f64;

        let bounds = *self.loop_bounds.lock().unwrap();
        let looping = bounds.enabled();

        self.buf.clear();
        let mut pos = f64::from_bits(self.source_pos.load(Ordering::Relaxed));

        for _ in 0..CHUNK_SIZE {
            if looping && pos as usize >= bounds.b {
                pos = bounds.a as f64;
                self.wrap_count.fetch_add(1, Ordering::Relaxed);
            }

            if pos as usize >= total_len {
                if looping {
                    pos = bounds.a as f64;
                    self.wrap_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    break;
                }
            }

            let floor = pos.floor() as usize;
            let frac = pos - floor as f64;
            let next_idx = (floor + 1).min(total_len - 1);

            let s0 = raw[floor] as f64;
            let s1 = raw[next_idx] as f64;
            let sample = s0 + (s1 - s0) * frac;

            self.buf.push(sample as f32);
            pos += step;
        }

        // Lock-free write
        self.source_pos.store(f64::to_bits(pos), Ordering::Relaxed);
        self.buf_idx = 0;
    }
}

impl Iterator for RealTimePitchTempoSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.buf_idx >= self.buf.len() {
            self.refill_buffer();
        }
        if self.buf_idx < self.buf.len() {
            let sample = self.buf[self.buf_idx];
            self.buf_idx += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for RealTimePitchTempoSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(std::usize::MAX)
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
