use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

pub enum PlayerCommand {
    PlayPause,

    Skip,
    AppendToQueue(Vec<String>),
    ReplaceQueue(Vec<String>),
    ReconnectAudio, // NIEUW: Commando om audio-apparaat te hervatten
}

pub enum PlayerEvent {
    NowPlaying(String),
}

pub fn run_audio_thread(rx: Receiver<PlayerCommand>, event_tx: Sender<PlayerEvent>) {
    // We stoppen de stream en sink in Options zodat we ze kunnen droppen en opnieuw maken
    let mut _stream_data: Option<(OutputStream, rodio::OutputStreamHandle)> = None;
    let mut sink: Option<Sink> = None;
    let mut internal_queue: Vec<String> = Vec::new();

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
                PlayerCommand::ReplaceQueue(files) => {
                    internal_queue = files;
                    if let Some(s) = &sink {
                        s.clear(); // Leeg de rodio wachtrij zodat hij niet doorspeelt
                        s.skip_one(); // Forceer direct naar de nieuwe lijst
                    }
                }
                PlayerCommand::AppendToQueue(files) => {
                    internal_queue.extend(files);
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
                if !internal_queue.is_empty() {
                    let next_file = internal_queue.remove(0);
                    if let Ok(f) = File::open(&next_file) {
                        if let Ok(decoder) = Decoder::new(BufReader::new(f)) {
                            s.append(decoder);
                            s.play();
                            let _ = event_tx.send(PlayerEvent::NowPlaying(next_file));
                        }
                    }
                }
            }
        }

        // Korte pauze om CPU te besparen
        std::thread::sleep(Duration::from_millis(50));
    }
}
