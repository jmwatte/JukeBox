#![windows_subsystem = "windows"]
#![allow(dead_code)]

mod config;
mod models;
mod player;
mod scanner;
mod search;
mod ui;
use crossbeam_channel::unbounded;

fn main() -> Result<(), eframe::Error> {
    let app_config = config::Config::load_or_create();

    // Kanalen voor communicatie met de audio-speler
    let (player_tx, player_rx) = unbounded();
    let (player_event_tx, player_event_rx) = unbounded(); // NIEUW: Retourkanaal

    // Kanalen voor de scanner
    let (scanner_tx, scanner_rx) = unbounded();

    // Start de audio thread
    std::thread::spawn(move || {
        player::run_audio_thread(player_rx, player_event_tx);
    });

    let music_dir = app_config.music_directory.clone();
    let audio_exts = app_config.audio_extensions.clone();
    let cover_names = app_config.cover_names.clone();
    let cover_exts = app_config.cover_extensions.clone();

    // Kloon de zender voor de initiële achtergrondscan
    let scanner_tx_bg = scanner_tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            scanner::load_or_scan_library(
                music_dir,
                audio_exts,
                cover_names,
                cover_exts,
                scanner_tx_bg,
            )
            .await;
        });
    });

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([
                app_config.window_size[0] as f32,
                app_config.window_size[1] as f32,
            ])
            .with_decorations(true)
            .with_resizable(true)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_native(
        "JukeBox",
        options,
        Box::new(move |_cc| {
            egui_extras::install_image_loaders(&_cc.egui_ctx);
            Ok(Box::new(ui::MusicPlayerApp::new(
                app_config,
                player_tx,
                player_event_rx, // NIEUW
                scanner_tx,
                scanner_rx,
            )))
        }),
    )
}
