use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatMode {
    None,
    One,
    All,
}

pub enum PlayerCommand {
    PlayPause,

    Skip,
    Rewind,
    Forward,
    ToggleRepeat,
    ToggleShuffle,
    RemoveFromQueue(usize),
    ClearQueue,
    SetLoopA,
    SetLoopB,
    SetLoopAAt(f32),
    SetLoopBAt(f32),
    ClearLoop,
    AppendToQueue(Vec<String>),
    ReplaceQueue(Vec<String>),
    SetVolume(f32),
    ReconnectAudio, // NIEUW: Commando om audio-apparaat te hervatten
}

pub enum PlayerEvent {
    NowPlaying(String),
    PositionUpdate(f32, f32), // (current_secs, total_secs)
    RepeatModeChanged(RepeatMode),
    ShuffleModeChanged(bool),
    QueueChanged(Vec<String>),             // (overige tracks in wachtrij)
    LoopChanged(Option<f32>, Option<f32>), // (A_secs, B_secs)
    PlaybackError(String),
}

pub fn run_audio_thread(rx: Receiver<PlayerCommand>, event_tx: Sender<PlayerEvent>) {
    // We stoppen de stream en sink in Options zodat we ze kunnen droppen en opnieuw maken
    let mut _stream_data: Option<(OutputStream, rodio::OutputStreamHandle)> = None;
    let mut sink: Option<Sink> = None;
    let mut internal_queue: Vec<String> = Vec::new();
    let mut current_track_duration: Option<Duration> = None;
    let mut repeat_mode = RepeatMode::None;
    let mut shuffle_on = false;
    let mut loop_a: Option<Duration> = None;
    let mut loop_b: Option<Duration> = None;
    let mut original_queue: Vec<String> = Vec::new();
    let mut last_track: Option<String> = None;

    // Eerste verbinding bij het opstarten (INLINE, geen closure!)
    if let Ok((stream, handle)) = OutputStream::try_default() {
        if let Ok(new_sink) = Sink::try_new(&handle) {
            _stream_data = Some((stream, handle));
            sink = Some(new_sink);
            println!("Audio device connected.");
        }
    }

    loop {
        // 1. Verwerk UI Commando's
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::PlayPause => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                        } else {
                            s.pause();
                        }
                    }
                }

                PlayerCommand::Skip => {
                    if let Some(s) = &sink {
                        s.skip_one();
                    }
                }
                PlayerCommand::Rewind => {
                    if let Some(s) = &sink {
                        let pos = s.get_pos();
                        let new_pos = pos.saturating_sub(Duration::from_secs(2));
                        let _ = s.try_seek(new_pos);
                    }
                }
                PlayerCommand::Forward => {
                    if let Some(s) = &sink {
                        let pos = s.get_pos();
                        let new_pos = pos + Duration::from_secs(2);
                        if let Some(dur) = current_track_duration {
                            if new_pos < dur {
                                let _ = s.try_seek(new_pos);
                            }
                        } else {
                            let _ = s.try_seek(new_pos);
                        }
                    }
                }
                PlayerCommand::ToggleRepeat => {
                    repeat_mode = match repeat_mode {
                        RepeatMode::None => RepeatMode::One,
                        RepeatMode::One => RepeatMode::All,
                        RepeatMode::All => RepeatMode::None,
                    };
                    let _ = event_tx.send(PlayerEvent::RepeatModeChanged(repeat_mode));
                }
                PlayerCommand::ToggleShuffle => {
                    shuffle_on = !shuffle_on;
                    let _ = event_tx.send(PlayerEvent::ShuffleModeChanged(shuffle_on));
                }
                PlayerCommand::RemoveFromQueue(idx) => {
                    if idx < internal_queue.len() {
                        internal_queue.remove(idx);
                        let _ = event_tx.send(PlayerEvent::QueueChanged(internal_queue.clone()));
                    }
                }
                PlayerCommand::ClearQueue => {
                    internal_queue.clear();
                    let _ = event_tx.send(PlayerEvent::QueueChanged(Vec::new()));
                }
                PlayerCommand::SetLoopA => {
                    if let Some(s) = &sink {
                        loop_a = Some(s.get_pos());
                        let a = loop_a.map(|d| d.as_secs_f32());
                        let b = loop_b.map(|d| d.as_secs_f32());
                        let _ = event_tx.send(PlayerEvent::LoopChanged(a, b));
                    }
                }
                PlayerCommand::SetLoopB => {
                    if let Some(s) = &sink {
                        loop_b = Some(s.get_pos());
                        // Als B voor A ligt, wissel ze om
                        if let (Some(a), Some(b)) = (loop_a, loop_b) {
                            if b < a {
                                loop_a = Some(b);
                                loop_b = Some(a);
                            }
                        }
                        let a = loop_a.map(|d| d.as_secs_f32());
                        let b = loop_b.map(|d| d.as_secs_f32());
                        let _ = event_tx.send(PlayerEvent::LoopChanged(a, b));
                    }
                }
                PlayerCommand::SetLoopAAt(secs) => {
                    loop_a = Some(Duration::from_secs_f32(secs));
                    let a = loop_a.map(|d| d.as_secs_f32());
                    let b = loop_b.map(|d| d.as_secs_f32());
                    let _ = event_tx.send(PlayerEvent::LoopChanged(a, b));
                }
                PlayerCommand::SetLoopBAt(secs) => {
                    loop_b = Some(Duration::from_secs_f32(secs));
                    if let (Some(a), Some(b)) = (loop_a, loop_b) {
                        if b < a {
                            loop_a = Some(b);
                            loop_b = Some(a);
                        }
                    }
                    let a = loop_a.map(|d| d.as_secs_f32());
                    let b = loop_b.map(|d| d.as_secs_f32());
                    let _ = event_tx.send(PlayerEvent::LoopChanged(a, b));
                }
                PlayerCommand::ClearLoop => {
                    loop_a = None;
                    loop_b = None;
                    let _ = event_tx.send(PlayerEvent::LoopChanged(None, None));
                }
                PlayerCommand::ReplaceQueue(files) => {
                    loop_a = None;
                    loop_b = None;
                    let _ = event_tx.send(PlayerEvent::LoopChanged(None, None));
                    original_queue = files.clone();
                    internal_queue = files;
                    if shuffle_on {
                        shuffle_vec(&mut internal_queue);
                    }
                    if let Some(s) = &sink {
                        s.clear(); // Leeg de rodio wachtrij zodat hij niet doorspeelt
                        s.skip_one(); // Forceer direct naar de nieuwe lijst
                    }
                    let _ = event_tx.send(PlayerEvent::QueueChanged(internal_queue.clone()));
                }
                PlayerCommand::AppendToQueue(files) => {
                    internal_queue.extend(files);
                    let _ = event_tx.send(PlayerEvent::QueueChanged(internal_queue.clone()));
                }
                PlayerCommand::SetVolume(vol) => {
                    if let Some(s) = &sink {
                        s.set_volume(vol);
                    }
                }
                PlayerCommand::ReconnectAudio => {
                    println!("Reconnecting audio device...");

                    // Drop de oude verbinding door de Options op None te zetten
                    sink = None;
                    _stream_data = None;

                    // Maak een nieuwe verbinding (INLINE)
                    if let Ok((stream, handle)) = OutputStream::try_default() {
                        if let Ok(new_sink) = Sink::try_new(&handle) {
                            _stream_data = Some((stream, handle));
                            sink = Some(new_sink);
                            println!("Audio device reconnected.");
                        } else {
                            eprintln!("Failed to create new sink.");
                        }
                    } else {
                        eprintln!("Failed to connect to new audio device.");
                    }
                }
            }
        }

        // 2. Beheer de weergave
        if let Some(s) = &sink {
            if s.empty() {
                // Herhaalmodus: vul queue opnieuw als deze leeg is
                if internal_queue.is_empty() {
                    match repeat_mode {
                        RepeatMode::One => {
                            if let Some(ref track) = last_track {
                                internal_queue.push(track.clone());
                                let _ = event_tx
                                    .send(PlayerEvent::QueueChanged(internal_queue.clone()));
                            }
                        }
                        RepeatMode::All => {
                            internal_queue = original_queue.clone();
                            if shuffle_on {
                                shuffle_vec(&mut internal_queue);
                            }
                            let _ =
                                event_tx.send(PlayerEvent::QueueChanged(internal_queue.clone()));
                        }
                        RepeatMode::None => {}
                    }
                }

                if !internal_queue.is_empty() {
                    let next_file = internal_queue.remove(0);
                    match File::open(&next_file) {
                        Ok(f) => match Decoder::new(BufReader::new(f)) {
                            Ok(decoder) => {
                                current_track_duration = decoder.total_duration();
                                last_track = Some(next_file.clone());
                                s.append(decoder);
                                s.play();
                                let _ = event_tx.send(PlayerEvent::NowPlaying(next_file));
                                let _ = event_tx
                                    .send(PlayerEvent::QueueChanged(internal_queue.clone()));
                            }
                            Err(e) => {
                                let msg = format!(
                                    "Kan bestand niet decoderen: {} ({})",
                                    std::path::Path::new(&next_file)
                                        .file_name()
                                        .map(|n| n.to_string_lossy())
                                        .unwrap_or_else(|| std::borrow::Cow::from(&next_file)),
                                    e
                                );
                                let _ = event_tx.send(PlayerEvent::PlaybackError(msg));
                            }
                        },
                        Err(e) => {
                            let msg = format!(
                                "Kan bestand niet openen: {} ({})",
                                std::path::Path::new(&next_file)
                                    .file_name()
                                    .map(|n| n.to_string_lossy())
                                    .unwrap_or_else(|| std::borrow::Cow::from(&next_file)),
                                e
                            );
                            let _ = event_tx.send(PlayerEvent::PlaybackError(msg));
                        }
                    }
                }
            }
        }

        // 3. A-B loop: als positie >= B, spring terug naar A
        if let (Some(a), Some(b)) = (loop_a, loop_b) {
            if let Some(s) = &sink {
                if !s.empty() && s.get_pos() >= b {
                    let _ = s.try_seek(a);
                }
            }
        }

        // 4. Stuur positie-update (als er iets speelt)
        if let Some(s) = &sink {
            if !s.empty() {
                let pos = s.get_pos().as_secs_f32();
                let dur = current_track_duration
                    .map(|d| d.as_secs_f32())
                    .unwrap_or(0.0);
                let _ = event_tx.send(PlayerEvent::PositionUpdate(pos, dur));
            }
        }

        // Korte pauze om CPU te besparen
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Eenvoudige Fisher-Yates shuffle met SystemTime als seed
fn shuffle_vec<T>(vec: &mut Vec<T>) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut rng = seed;
    let len = vec.len();
    for i in (1..len).rev() {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (rng % (i as u128 + 1)) as usize;
        vec.swap(i, j);
    }
}
