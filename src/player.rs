use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

pub enum PlayerCommand {
    PlayPause,
    Stop,
    Skip,
    AppendToQueue(Vec<String>),
    ReplaceQueue(Vec<String>),
}

// NIEUW: De berichten die de speler TERUG stuurt naar de UI
pub enum PlayerEvent {
    NowPlaying(String),
    Stopped,
}

pub fn run_audio_thread(rx: Receiver<PlayerCommand>, event_tx: Sender<PlayerEvent>) {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // De speler houdt nu zelf zijn wachtrij bij zodat we weten wat het volgende nummer is
    let mut internal_queue: Vec<String> = Vec::new();

    loop {
        // 1. Verwerk UI Commando's
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::PlayPause => {
                    if sink.is_paused() {
                        sink.play();
                    } else {
                        sink.pause();
                    }
                }
                PlayerCommand::Stop => {
                    internal_queue.clear();
                    sink.skip_one(); // Stopt de huidige audio
                    let _ = event_tx.send(PlayerEvent::Stopped);
                }
                PlayerCommand::Skip => {
                    sink.skip_one(); // Stopt huidige, de loop pakt automatisch de volgende
                }
                PlayerCommand::ReplaceQueue(files) => {
                    internal_queue = files;
                    sink.skip_one(); // Forceer direct naar de nieuwe lijst
                }
                PlayerCommand::AppendToQueue(files) => {
                    internal_queue.extend(files);
                }
            }
        }

        // 2. Beheer de weergave
        if sink.empty() {
            if !internal_queue.is_empty() {
                let next_file = internal_queue.remove(0); // Pak de eerste uit de rij
                if let Ok(f) = File::open(&next_file) {
                    if let Ok(decoder) = Decoder::new(BufReader::new(f)) {
                        sink.append(decoder);
                        sink.play();
                        // Laat de UI weten wat we nu spelen!
                        let _ = event_tx.send(PlayerEvent::NowPlaying(next_file));
                    }
                }
            }
        }

        // Korte pauze om CPU te besparen
        std::thread::sleep(Duration::from_millis(50));
    }
}
