#![windows_subsystem = "windows"]

mod config;
mod favorites;
mod loops;
mod models;
mod player;
mod scanner;
mod search;
mod ui;
mod waveform;
use crossbeam_channel::unbounded;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("JukeBoks gestart");
    let app_config = config::Config::load_or_create();

    // Kanalen voor communicatie met de audio-speler
    let (player_tx, player_rx) = unbounded();
    let (player_event_tx, player_event_rx) = unbounded();

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
        scanner::load_or_scan_library(
            music_dir,
            audio_exts,
            cover_names,
            cover_exts,
            scanner_tx_bg,
        );
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
        "JukeBoks",
        options,
        Box::new(move |_cc| {
            egui_extras::install_image_loaders(&_cc.egui_ctx);
            // Voeg Windows-systeemlettertypen toe als fallback voor symbolen/emoji
            // die niet in de gebundelde egui-lettertypen zitten (anders zie je
            // "□"-blokjes, bijv. voor ⟲, ⏹, ◢).
            install_fallback_fonts(&_cc.egui_ctx);
            Ok(Box::new(ui::MusicPlayerApp::new(
                app_config,
                player_tx,
                player_event_rx,
                scanner_tx,
                scanner_rx,
            )))
        }),
    )
}

/// Voeg Windows-systeemlettertypen toe als fallback (alleen gebruikt voor glyphs
/// die in géén eerder lettertype zitten, dus de gewone tekst verandert niet).
fn install_fallback_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    use eframe::egui::{FontData, FontFamily};

    let candidates = [
        ("C:\\Windows\\Fonts\\seguisym.ttf", "Segoe UI Symbol"),
        ("C:\\Windows\\Fonts\\seguiemj.ttf", "Segoe UI Emoji"),
    ];
    for (path, name) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.add_font(FontInsert::new(
                name,
                FontData::from_owned(bytes),
                vec![
                    InsertFontFamily {
                        family: FontFamily::Proportional,
                        priority: FontPriority::Lowest,
                    },
                    InsertFontFamily {
                        family: FontFamily::Monospace,
                        priority: FontPriority::Lowest,
                    },
                ],
            ));
        }
    }
}
