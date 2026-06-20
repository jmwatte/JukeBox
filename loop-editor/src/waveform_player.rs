use crossbeam_channel::{Receiver, Sender};
use rodio::{OutputStream, Sink, Source};
use soundtouch::SoundTouch;
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
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
    },
    Stop,
    #[allow(dead_code)]
    Pause,
    #[allow(dead_code)]
    Resume,
    TogglePause,
    SetLoopBounds {
        a_secs: f32,
        b_secs: f32,
    },
    Seek {
        pos_secs: f32,
    },
    SetPitch(f32),
    SetTempo(f32),
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
// Interne audio-loop
// ---------------------------------------------------------------------------
fn run_waveform_audio(rx: Receiver<WaveformCommand>, event_tx: Sender<WaveformEvent>) {
    let mut _stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut is_playing = false;
    let mut is_paused = false;

    // Lock-free shared state
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

                    wrap_count.store(0, Ordering::Relaxed);
                    prev_wrap = 0;
                    wrap_limit_sent = false;

                    let source = SoundTouchSource::new(
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
                        // *segment_start_sec.lock().unwrap() = a_secs;
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
                    source_pos.store(f64::to_bits(sample), Ordering::Relaxed);
                }
                WaveformCommand::SetPitch(semitones) => {
                    pitch_semitones.store(f32::to_bits(semitones), Ordering::Relaxed);
                }
                WaveformCommand::SetTempo(new_tempo) => {
                    tempo.store(f32::to_bits(new_tempo), Ordering::Relaxed);
                }
                WaveformCommand::SetLoopEnabled(enabled) => {
                    let mut bounds = loop_bounds.lock().unwrap();
                    bounds.enabled = enabled;
                }
            }
        }

        if is_playing && !is_paused {
            if let Some(s) = &sink {
                if s.empty() {
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                } else {
                    // Lock-free: atomic read
                    let pos_samples = f64::from_bits(source_pos.load(Ordering::Relaxed));
                    let sr = *current_sample_rate.lock().unwrap();
                    let bounds = *loop_bounds.lock().unwrap();

                    // ✅ Nieuwe totale duur is de lengte van de volledige track buffer
                    let total_dur = samples.lock().unwrap().len() as f32 / sr as f32;

                    // Absolute tijd in de track
                    let pos_secs = pos_samples as f32 / sr as f32;

                    let effective_pos = if bounds.enabled() {
                        let loop_start_sec = bounds.a as f32 / sr as f32;
                        let loop_end_sec = bounds.b as f32 / sr as f32;
                        let loop_dur = loop_end_sec - loop_start_sec;

                        // ✅ Correcte wrap-wiskunde voor absolute tijden
                        if loop_dur > 0.0 && pos_secs >= loop_end_sec {
                            loop_start_sec + ((pos_secs - loop_start_sec) % loop_dur)
                        } else {
                            pos_secs
                        }
                    } else {
                        pos_secs
                    };

                    let _ = event_tx.send(WaveformEvent::Position(effective_pos, total_dur));

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
// SoundTouchSource — Industry Standard DSP via SoundTouch
// ---------------------------------------------------------------------------
struct SoundTouchSource {
    raw_samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    pitch_semitones: Arc<AtomicU32>,
    tempo: Arc<AtomicU32>,
    loop_bounds: Arc<Mutex<LoopBounds>>,
    source_pos: Arc<AtomicU64>,
    wrap_count: Arc<AtomicU32>,

    st: SoundTouch,
    read_pos: usize,
    out_buf: Vec<f32>,
    out_idx: usize,

    current_pitch: f32,
    current_tempo: f32,
}

impl SoundTouchSource {
    fn new(
        raw_samples: Arc<Mutex<Vec<f32>>>,
        sample_rate: u32,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
        loop_bounds: Arc<Mutex<LoopBounds>>,
        source_pos: Arc<AtomicU64>,
        wrap_count: Arc<AtomicU32>,
    ) -> Self {
        let mut st = SoundTouch::new();
        st.set_sample_rate(sample_rate);
        st.set_channels(1); // Mono

        let initial_pitch = f32::from_bits(pitch_semitones.load(Ordering::Relaxed));
        let initial_tempo = f32::from_bits(tempo.load(Ordering::Relaxed));

        // ✅ FIX 1: Gebruik set_pitch met een ratio i.p.v. set_pitch_semitones
        let pitch_ratio = f64::powf(2.0, (initial_pitch as f64) / 12.0);
        st.set_pitch(pitch_ratio);

        // ✅ FIX 2: Cast tempo naar f64
        st.set_tempo(initial_tempo as f64);

        let start_pos = f64::from_bits(source_pos.load(Ordering::Relaxed)) as usize;

        Self {
            raw_samples,
            sample_rate,
            pitch_semitones,
            tempo,
            loop_bounds,
            source_pos,
            wrap_count,
            st,
            read_pos: start_pos,
            out_buf: Vec::with_capacity(4096),
            out_idx: 0,
            current_pitch: initial_pitch,
            current_tempo: initial_tempo,
        }
    }

    fn fill_buffer(&mut self) {
        // ✅ FIX: Detecteer of de UI een Seek commando heeft gestuurd (bijv. pijltjestoetsen)
        // We lezen de Atomic source_pos. Als deze afwijkt van onze interne read_pos,
        // is er geseekt. We gebruiken een drempel van 10 samples om floating-point
        // afrondingsfouten te negeren.
        let atomic_pos = f64::from_bits(self.source_pos.load(Ordering::Relaxed));
        if (atomic_pos - self.read_pos as f64).abs() > 10.0 {
            // De UI heeft geseekt! Trek onze interne leespositie bij.
            self.read_pos = atomic_pos as usize;
        }

        self.out_buf.clear();
        self.out_idx = 0;

        // Update SoundTouch parameters als de UI ze heeft gewijzigd
        let new_pitch = f32::from_bits(self.pitch_semitones.load(Ordering::Relaxed));
        let new_tempo = f32::from_bits(self.tempo.load(Ordering::Relaxed));

        if (new_pitch - self.current_pitch).abs() > 0.01 {
            let pitch_ratio = f64::powf(2.0, (new_pitch as f64) / 12.0);
            self.st.set_pitch(pitch_ratio);
            self.current_pitch = new_pitch;
        }
        if (new_tempo - self.current_tempo).abs() > 0.01 {
            self.st.set_tempo(new_tempo as f64);
            self.current_tempo = new_tempo;
        }

        let raw = self.raw_samples.lock().unwrap();
        let bounds = self.loop_bounds.lock().unwrap();
        let looping = bounds.enabled();
        let total_len = raw.len();

        if total_len == 0 {
            return;
        }

        let target_out = 4096;
        let mut input_chunk = Vec::with_capacity(4096);

        while self.out_buf.len() < target_out {
            input_chunk.clear();

            // 1. Verzamel ruwe input samples (respecteer loops)
            while input_chunk.len() < 4096 {
                // ✅ FIX: Bepaal de harde grens waar we maximaal tot mogen lezen
                let end_pos = if looping { bounds.b } else { total_len };

                // ✅ FIX: Als we de grens bereiken (of passeren), wrap direct terug naar A
                if self.read_pos >= end_pos {
                    if looping {
                        self.read_pos = bounds.a;
                        self.wrap_count.fetch_add(1, Ordering::Relaxed);
                        continue; // Ga opnieuw door de loop om de nieuwe chunk te lezen
                    } else {
                        break; // Einde van de track (geen loop)
                    }
                }

                // Bereken hoeveel samples we in dit stukje kunnen lezen
                let to_read = (4096 - input_chunk.len()).min(end_pos - self.read_pos);

                input_chunk.extend_from_slice(&raw[self.read_pos..self.read_pos + to_read]);
                self.read_pos += to_read;
            }

            if input_chunk.is_empty() {
                // Einde van audio, flush resterende DSP buffer
                self.st.flush();
                let mut flush_buf = vec![0.0; 4096];
                let received = self.st.receive_samples(&mut flush_buf, 4096);
                if received > 0 {
                    self.out_buf.extend_from_slice(&flush_buf[..received]);
                }
                break;
            }

            // 2. Feed into SoundTouch
            self.st.put_samples(&input_chunk, input_chunk.len());

            // 3. Extract processed audio
            let mut temp_out = vec![0.0; 4096];
            let received = self.st.receive_samples(&mut temp_out, 4096);
            if received > 0 {
                self.out_buf.extend_from_slice(&temp_out[..received]);
            } else if !looping && self.read_pos >= total_len {
                self.st.flush();
                let received = self.st.receive_samples(&mut temp_out, 4096);
                if received > 0 {
                    self.out_buf.extend_from_slice(&temp_out[..received]);
                }
                break;
            }
        }

        // Update UI playhead positie
        self.source_pos
            .store(f64::to_bits(self.read_pos as f64), Ordering::Relaxed);
    }
}

impl Iterator for SoundTouchSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.out_idx >= self.out_buf.len() {
            self.fill_buffer();
        }
        if self.out_idx < self.out_buf.len() {
            let val = self.out_buf[self.out_idx];
            self.out_idx += 1;
            Some(val)
        } else {
            None
        }
    }
}

impl Source for SoundTouchSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(usize::MAX)
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
