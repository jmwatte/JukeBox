#![windows_subsystem = "windows"]

mod app;
mod chroma;
mod loops;
mod shortcuts;
mod timestretch;
mod waveform;
mod waveform_player;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_native(
        "Waveform Loop Editor",
        options,
        Box::new(|_cc| Ok(Box::new(app::LoopEditorApp::new()))),
    )
}
