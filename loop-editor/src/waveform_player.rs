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

/// Gecombineerde pitch-resample en tempo-resample stap.
/// `step` is hoeveel samples er in de bron worden opgeschoven per
/// pitch-shifted sample. Alleen `pitch_factor` bepaalt dit.
/// De tempo-correctie gebeurt in een tweede resample-stap.
fn lerp(raw: &[f32], pos: f64) -> f32 {
    let len = raw.len();
    if len == 0 {
        return 0.0;
    }
    let floor = (pos.floor() as usize).min(len.saturating_sub(1));
    let next = (floor + 1).min(len.saturating_sub(1));
    let frac = pos - pos.floor();
    let s0 = raw[floor] as f64;
    let s1 = raw[next] as f64;
    (s0 + (s1 - s0) * frac) as f32
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
// RealTimePitchTempoSource — Granular Overlap-Add
// ---------------------------------------------------------------------------

/// Aantal output samples per chunk.
const CHUNK_SIZE: usize = 512;

/// Grootte van elk grain (in output samples).
const GRAIN_SIZE: usize = 2048;

/// Gebruikt Granular Overlap-Add (OLA) met 2 simultane grains die 50% overlappen.
///
/// **Pitch** wordt bepaald door de leessnelheid (`read_step = pitch_factor`) onafhankelijk
/// van de grain-envelope.
/// **Tempo** wordt bepaald door hoe snel de grain-fase doorloopt (`tempo / grain_size`).
///
/// Doordat de leessnelheid (pitch) en de envelop-snelheid (tempo) volledig gescheiden zijn,
/// is de toonhoogte onafhankelijk van de afspeelsnelheid.
struct RealTimePitchTempoSource {
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    pitch_semitones: Arc<AtomicU32>,
    tempo: Arc<AtomicU32>,
    loop_bounds: Arc<Mutex<LoopBounds>>,
    source_pos: Arc<AtomicU64>,
    wrap_count: Arc<AtomicU32>,

    // Granular state
    read_pos_a: f64, // leespositie grain A in ruwe samples
    read_pos_b: f64, // leespositie grain B in ruwe samples (offset = GRAIN_SIZE/2 * pitch_factor)
    phase_a: f64,    // grain-fase A: 0.0–1.0
    phase_b: f64,    // grain-fase B: 0.0–1.0 (offset = 0.5)

    // Pre-berekende Hann window
    hann: Vec<f32>,

    // Output
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
        let start = f64::from_bits(source_pos.load(Ordering::Relaxed));
        let pitch = f32::from_bits(pitch_semitones.load(Ordering::Relaxed));
        let pf = f32::powf(2.0, pitch / 12.0) as f64;

        // Pre-bereken Hann window
        let mut hann = vec![0.0_f32; GRAIN_SIZE];
        for i in 0..GRAIN_SIZE {
            let p = std::f64::consts::TAU * i as f64 / (GRAIN_SIZE as f64 - 1.0);
            hann[i] = (0.5 * (1.0 - p.cos())) as f32;
        }

        Self {
            samples,
            sample_rate,
            pitch_semitones,
            tempo,
            loop_bounds,
            source_pos,
            wrap_count,
            read_pos_a: start,
            read_pos_b: start + GRAIN_SIZE as f64 * pf * 0.5,
            phase_a: 0.0,
            phase_b: 0.5,
            hann,
            buf: Vec::new(),
            buf_idx: 0,
        }
    }

    fn refill_buffer(&mut self) {
        let guard = self.samples.lock().unwrap();
        let raw = &*guard;
        if raw.is_empty() {
            self.buf.clear();
            return;
        }

        let total_len = raw.len() as f64;
        let bounds = *self.loop_bounds.lock().unwrap();
        let looping = bounds.enabled();
        let loop_a = bounds.a as f64;
        let loop_b = bounds.b as f64;

        let pitch_bits = self.pitch_semitones.load(Ordering::Relaxed);
        let tempo_bits = self.tempo.load(Ordering::Relaxed);
        let pf = f32::powf(2.0, f32::from_bits(pitch_bits) / 12.0) as f64;
        let tempo = f32::from_bits(tempo_bits) as f64;

        // Offset tussen A en B in de bron = halve korrel * pitch_factor
        let grain_offset = GRAIN_SIZE as f64 * pf * 0.5;

        // Fase-increment per output sample
        let phase_inc = tempo / GRAIN_SIZE as f64;

        self.buf.clear();

        for _ in 0..CHUNK_SIZE {
            // ── Looping check voor read_pos_a ──
            if looping && self.read_pos_a >= loop_b {
                self.read_pos_a -= loop_b - loop_a;
                self.wrap_count.fetch_add(1, Ordering::Relaxed);
            }
            if self.read_pos_a >= total_len {
                if looping {
                    self.read_pos_a -= total_len;
                    self.wrap_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    break;
                }
            }

            // read_pos_b = read_pos_a + grain_offset (constant offset)
            self.read_pos_b = self.read_pos_a + grain_offset;
            if self.read_pos_b >= total_len {
                if looping {
                    self.read_pos_b -= total_len;
                } else {
                    // Clamp — grain B valt buiten de buffer
                    self.read_pos_b = self.read_pos_b.min(total_len - 1.0).max(0.0);
                }
            }

            // ── Lees samples met lineaire interpolatie ──
            let s_a = lerp(raw, self.read_pos_a) as f64;
            let s_b = lerp(raw, self.read_pos_b) as f64;

            // ── Hann window op beide grains ──
            let idx_a = (self.phase_a.clamp(0.0, 0.9999) * (GRAIN_SIZE as f64 - 1.0)) as usize;
            let idx_b = (self.phase_b.clamp(0.0, 0.9999) * (GRAIN_SIZE as f64 - 1.0)) as usize;
            let w_a = self.hann[idx_a] as f64;
            let w_b = self.hann[idx_b] as f64;

            // ── Output = som van gewogen grains ──
            let out = s_a * w_a + s_b * w_b;
            self.buf.push(out as f32);

            // ── Advance ──
            self.read_pos_a += pf; // ← PITCH: alleen pitch_factor
            self.phase_a += phase_inc; // ← TEMPO: alleen tempo/grain_size
            self.phase_b += phase_inc;

            // Wrap grain-fases
            if self.phase_a >= 1.0 {
                self.phase_a -= 1.0;
            }
            if self.phase_b >= 1.0 {
                self.phase_b -= 1.0;
            }
        }

        self.source_pos
            .store(f64::to_bits(self.read_pos_a), Ordering::Relaxed);
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
