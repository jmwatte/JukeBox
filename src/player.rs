use crossbeam_channel::{Receiver, Sender};
use rand::rng;
use rand::seq::SliceRandom;
use rodio::{Decoder, Player, Source};
use std::fs::File;
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
    SeekTo(f32),
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
    let mut _stream_data: Option<rodio::MixerDeviceSink> = None;
    let mut sink: Option<Player> = None;
    let mut internal_queue: Vec<String> = Vec::new();
    let mut current_track_duration: Option<Duration> = None;
    let mut repeat_mode = RepeatMode::None;
    let mut shuffle_on = false;
    let mut loop_a: Option<Duration> = None;
    let mut loop_b: Option<Duration> = None;
    let mut original_queue: Vec<String> = Vec::new();
    let mut last_track: Option<String> = None;
    // Houdt de laatste seek positie bij, nodig voor hervatten vanuit pauze
    let mut pending_seek: Option<Duration> = None;
    // Seek die pas mag worden toegepast als een (nieuwe) track speelt — rodio
    // laat een try_seek vallen zolang de sink leeg is, dus na ReplaceQueue
    // (Play Loop / Enter) moet de seek herhaald worden tot de track actief is.
    // (pos, resterende pogingen)
    let mut pending_start_seek: Option<(Duration, u32)> = None;

    // Eerste verbinding bij het opstarten (INLINE, geen closure!)
    if let Ok(handle) = rodio::DeviceSinkBuilder::open_default_sink() {
        let new_player = Player::connect_new(handle.mixer());
        _stream_data = Some(handle);
        sink = Some(new_player);
        log::info!("Audio device connected.");
    }

    loop {
        // 1. Verwerk UI Commando's
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::PlayPause => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            // Hervat: speel eerst af zodat rodio's interne thread
                            // wakker wordt, pas daarna seek naar pending positie.
                            s.play();
                            if let Some(seek) = pending_seek.take() {
                                let _ = s.try_seek(seek);
                                // Stuur direct een positie-update met de
                                // gezochte positie, zodat de UI niet terugspringt
                                // naar de oude pauzepositie.
                                let dur = current_track_duration
                                    .map(|d| d.as_secs_f32())
                                    .unwrap_or(0.0);
                                let _ = event_tx
                                    .send(PlayerEvent::PositionUpdate(seek.as_secs_f32(), dur));
                            }
                        } else {
                            s.pause();
                            pending_seek = None; // pauzeren wist pending seek
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
                        if !s.empty() {
                            let pos = s.get_pos();
                            let new_pos = pos.saturating_sub(Duration::from_secs(2));
                            match s.try_seek(new_pos) {
                                Ok(()) => {
                                    if pending_seek.is_some() {
                                        pending_seek = Some(new_pos);
                                    }
                                    let dur = current_track_duration
                                        .map(|d| d.as_secs_f32())
                                        .unwrap_or(0.0);
                                    let _ = event_tx.send(PlayerEvent::PositionUpdate(
                                        new_pos.as_secs_f32(),
                                        dur,
                                    ));
                                }
                                Err(e) => {
                                    log::warn!("Spoelen terug mislukt: {:?}", e);
                                    let _ = event_tx.send(PlayerEvent::PlaybackError(
                                        "Spoelen wordt niet ondersteund voor dit bestand.".into(),
                                    ));
                                }
                            }
                        }
                    }
                }
                PlayerCommand::Forward => {
                    if let Some(s) = &sink {
                        if !s.empty() {
                            let pos = s.get_pos();
                            let new_pos = pos + Duration::from_secs(2);
                            let in_bounds = if let Some(dur) = current_track_duration {
                                new_pos < dur
                            } else {
                                true
                            };
                            if in_bounds {
                                match s.try_seek(new_pos) {
                                    Ok(()) => {
                                        if pending_seek.is_some() {
                                            pending_seek = Some(new_pos);
                                        }
                                        let dur = current_track_duration
                                            .map(|d| d.as_secs_f32())
                                            .unwrap_or(0.0);
                                        let _ = event_tx.send(PlayerEvent::PositionUpdate(
                                            new_pos.as_secs_f32(),
                                            dur,
                                        ));
                                    }
                                    Err(e) => {
                                        log::warn!("Spoelen vooruit mislukt: {:?}", e);
                                        let _ = event_tx.send(PlayerEvent::PlaybackError(
                                            "Spoelen wordt niet ondersteund voor dit bestand."
                                                .into(),
                                        ));
                                    }
                                }
                            }
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
                    pending_start_seek = None;
                    let _ = event_tx.send(PlayerEvent::QueueChanged(Vec::new()));
                }
                PlayerCommand::SetLoopA => {
                    if let Some(s) = &sink {
                        if !s.empty() {
                            loop_a = Some(s.get_pos());
                            let a = loop_a.map(|d| d.as_secs_f32());
                            let b = loop_b.map(|d| d.as_secs_f32());
                            let _ = event_tx.send(PlayerEvent::LoopChanged(a, b));
                        }
                    }
                }
                PlayerCommand::SetLoopB => {
                    if let Some(s) = &sink {
                        if !s.empty() {
                            loop_b = Some(s.get_pos());
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
                    // Een verse queue start zonder bewaarde start-seek; alleen een
                    // SeekTo ná deze ReplaceQueue (Play Loop / Enter) zet hem weer.
                    pending_start_seek = None;
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
                PlayerCommand::SeekTo(pos) => {
                    let seek_pos = Duration::from_secs_f32(pos);
                    if let Some(s) = &sink {
                        if !s.empty() {
                            pending_seek = Some(seek_pos);
                            if let Some(dur) = current_track_duration {
                                if seek_pos < dur {
                                    let _ = s.try_seek(seek_pos);
                                }
                            } else {
                                let _ = s.try_seek(seek_pos);
                            }
                        } else {
                            // Sink is (nog) leeg — rodio laat de seek vallen.
                            // Bewaar hem en pas toe zodra de track speelt
                            // (belangrijk voor Play Loop / Enter: starten op A).
                            pending_start_seek = Some((seek_pos, 33));
                        }
                    }
                    // Stuur de GEVRAAGDE positie (s.get_pos() is onbetrouwbaar als gepauzeerd)
                    let dur = current_track_duration
                        .map(|d| d.as_secs_f32())
                        .unwrap_or(0.0);
                    let _ = event_tx.send(PlayerEvent::PositionUpdate(pos, dur));
                }
                PlayerCommand::ReconnectAudio => {
                    log::info!("Reconnecting audio device...");

                    // Drop de oude verbinding door de Options op None te zetten
                    sink = None;
                    _stream_data = None;

                    // Maak een nieuwe verbinding (INLINE)
                    if let Ok(handle) = rodio::DeviceSinkBuilder::open_default_sink() {
                        let new_player = Player::connect_new(handle.mixer());
                        _stream_data = Some(handle);
                        sink = Some(new_player);
                        log::info!("Audio device reconnected.");
                    } else {
                        log::error!("Failed to connect to new audio device.");
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
                        Ok(f) => match Decoder::try_from(f) {
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

        // 2b. Pending start-seek toepassen: zodra de (nieuwe) track speelt, de
        //     bewaarde seek alsnog uitvoeren (Play Loop / Enter starten op A).
        //     rodio laat een seek vallen zolang er geen sound actief is, dus
        //     controleer via get_pos() of de seek echt is toegepast en herhaal.
        if let Some((seek, attempts)) = pending_start_seek {
            if let Some(s) = &sink {
                if !s.empty() {
                    let _ = s.try_seek(seek);
                    let pos = s.get_pos();
                    let applied = (pos.as_secs_f32() - seek.as_secs_f32()).abs() < 0.5;
                    if applied {
                        pending_start_seek = None;
                    } else if attempts > 0 {
                        pending_start_seek = Some((seek, attempts - 1));
                    } else {
                        log::warn!(
                            "Kon start-seek naar {:.1}s niet toepassen (ongeschikt bestand?)",
                            seek.as_secs_f32()
                        );
                        pending_start_seek = None;
                    }
                }
            } else if attempts > 0 {
                pending_start_seek = Some((seek, attempts - 1));
            } else {
                pending_start_seek = None;
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
                // Gebruik pending_seek als die actief is (s.get_pos() is
                // onbetrouwbaar wanneer de sink gepauzeerd is)
                let pos = if let Some(seek) = pending_seek {
                    seek.as_secs_f32()
                } else {
                    s.get_pos().as_secs_f32()
                };
                let dur = current_track_duration
                    .map(|d| d.as_secs_f32())
                    .unwrap_or(0.0);
                let _ = event_tx.send(PlayerEvent::PositionUpdate(pos, dur));
            }
        }

        // Korte pauze om CPU te besparen — 30ms voor vloeiende positie-updates
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Fisher-Yates shuffle via `rand` crate
fn shuffle_vec<T>(vec: &mut Vec<T>) {
    vec.shuffle(&mut rng());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Regressietest voor de rodio-patch (byte_len in ReadSeekSource).
    ///
    /// Symphonia's FLAC-reader kan niet seaken zonder `byte_len()`:
    /// zonder de patch faalt elke seek met SeekErrorKind::Unseekable en
    /// toonde de app ten onrechte "Spoelen wordt niet ondersteund".
    #[test]
    fn symphonia_flac_seek_works() {
        // Kleine test-FLAC (1s sinus, gegenereerd met ffmpeg)
        let bytes = include_bytes!("../assets/seek_test.flac");
        let mut decoder = Decoder::new(Cursor::new(bytes.to_vec())).expect("FLAC decoderen");

        let total = decoder
            .total_duration()
            .expect("totale duur moet bekend zijn voor FLAC");
        assert!(total.as_secs() >= 1, "onverwachte totale duur: {:?}", total);

        // Seek naar halverwege het nummer — dit faalde vóór de byte_len-patch.
        let target = total / 2;
        match Source::try_seek(&mut decoder, target) {
            Ok(()) => {}
            Err(e) => panic!("seek naar {:?} faalt: {:?}", target, e),
        }

        // Na de seek moeten er nog samples leesbaar zijn.
        let mut read = 0usize;
        for _ in 0..(total.as_secs_f32() * 8000.0) as usize {
            if decoder.next().is_none() {
                break;
            }
            read += 1;
        }
        assert!(
            read > 1000,
            "na seek werden slechts {} samples gelezen",
            read
        );
    }
}
