use crate::config::Config;
use crate::models::Library;
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::filter_library;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, Image, Key, RichText, ScrollArea};
use std::path::Path;

#[derive(PartialEq, Clone)]
pub enum NavLevel {
    Artist,
    Album,
    Disk,
    Track,
}

#[derive(PartialEq, Clone)]
pub enum ViewMode {
    Tracklist,
    AlbumCover,
}

pub struct MusicPlayerApp {
    config: Config,
    player_tx: Sender<PlayerCommand>,
    player_event_rx: Receiver<PlayerEvent>, // Kanaal om 'Now Playing' te ontvangen
    scanner_tx: Sender<ScannerMessage>,
    scanner_rx: Receiver<ScannerMessage>,
    library: Option<Library>,

    // NIEUWE STATUS VARIABELEN
    now_playing: Option<String>,
    show_help: bool,

    _status_message: String,

    current_level: NavLevel,
    view_mode: ViewMode,
    selected_artist: usize,
    selected_album: usize,
    selected_disk: usize,
    selected_track: usize,
    scroll_to_selection: bool,
    search_query: String,
    //search_results: Vec<SearchResult>,
    // selected_search_index: usize,
    is_search_active: bool,
    search_input_id: egui::Id,
    filtered_library: Option<Library>,
}

impl MusicPlayerApp {
    pub fn new(
        config: Config,
        player_tx: Sender<PlayerCommand>,
        player_event_rx: Receiver<PlayerEvent>,
        scanner_tx: Sender<ScannerMessage>,
        scanner_rx: Receiver<ScannerMessage>,
    ) -> Self {
        let view_mode = if config.startup_view == "cover" {
            ViewMode::AlbumCover
        } else {
            ViewMode::Tracklist
        };
        Self {
            config,
            player_tx,
            player_event_rx,
            scanner_tx,
            scanner_rx,
            library: None,
            now_playing: None,
            show_help: false,
            _status_message: "Bibliotheek opstarten...".to_string(),
            filtered_library: None,
            // _search_active: false,
            search_query: String::new(),
            current_level: NavLevel::Artist,
            view_mode,
            selected_artist: 0,
            selected_album: 0,
            selected_disk: 0,
            selected_track: 0,
            scroll_to_selection: true,
            // search_query: String::new(),
            // search_results: Vec::new(),
            // selected_search_index: 0,
            is_search_active: false,
            search_input_id: egui::Id::new("global_search_input"),
        }
    }

    // fn active_library(&self) -> Option<&Library> {
    //     self.filtered_library.as_ref().or(self.library.as_ref())
    // }

    fn play_selected_item(&self, lib: &Library, replace: bool) {
        let mut queue = Vec::new();
        match self.current_level {
            NavLevel::Track => {
                let track = &lib.artists[self.selected_artist].albums[self.selected_album].disks
                    [self.selected_disk]
                    .tracks[self.selected_track];
                queue.push(track.path.clone());
            }
            NavLevel::Disk | NavLevel::Album | NavLevel::Artist => {
                let artist = &lib.artists[self.selected_artist];
                match self.current_level {
                    NavLevel::Artist => {
                        for album in &artist.albums {
                            for disk in &album.disks {
                                for track in &disk.tracks {
                                    queue.push(track.path.clone());
                                }
                            }
                        }
                    }
                    NavLevel::Album => {
                        for disk in &artist.albums[self.selected_album].disks {
                            for track in &disk.tracks {
                                queue.push(track.path.clone());
                            }
                        }
                    }
                    NavLevel::Disk => {
                        for track in
                            &artist.albums[self.selected_album].disks[self.selected_disk].tracks
                        {
                            queue.push(track.path.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        if !queue.is_empty() {
            if replace {
                let _ = self.player_tx.send(PlayerCommand::ReplaceQueue(queue));
            } else {
                let _ = self.player_tx.send(PlayerCommand::AppendToQueue(queue));
            }
        }
    }

    fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.is_search_active || self.filtered_library.is_some() {
                self.is_search_active = false;
                self.filtered_library = None;
                self.search_query.clear();
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
                return;
            }
        }

        // --- HELP SCHERM (?) of (H) ---
        if ctx.input(|i| {
            i.key_pressed(Key::H)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "?"))
        }) {
            self.show_help = !self.show_help;
        }

        // --- F5 RESCAN ---
        if ctx.input(|i| i.key_pressed(Key::F5)) {
            let _ = std::fs::remove_file("library_cache.bin");
            self.library = None;
            self.filtered_library = None;
            let tx = self.scanner_tx.clone();
            let config = self.config.clone();
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(async {
                        crate::scanner::load_or_scan_library(
                            config.music_directory,
                            config.audio_extensions,
                            config.cover_names,
                            config.cover_extensions,
                            tx,
                        )
                        .await;
                    });
                }
            });
            return;
        }

        let lib = self.filtered_library.as_ref().or(self.library.as_ref());
        let Some(lib) = lib else {
            return;
        };

        // --- R-TOETS RANDOM ALBUM ---
        if ctx.input(|i| i.key_pressed(Key::R)) {
            if !lib.artists.is_empty() {
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize;
                let random_artist = time % lib.artists.len();
                if !lib.artists[random_artist].albums.is_empty() {
                    let random_album = (time / 100) % lib.artists[random_artist].albums.len();
                    self.selected_artist = random_artist;
                    self.selected_album = random_album;
                    self.current_level = NavLevel::Album;
                    self.scroll_to_selection = true;
                }
            }
        }

        if ctx.input(|i| i.key_pressed(Key::T)) {
            self.view_mode = match self.view_mode {
                ViewMode::Tracklist => ViewMode::AlbumCover,
                ViewMode::AlbumCover => ViewMode::Tracklist,
            };
        }

        // --- AFSPEEL BESTURING ---
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            let _ = self.player_tx.send(PlayerCommand::PlayPause);
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            self.play_selected_item(lib, true);
        }
        if ctx.input(|i| i.key_pressed(Key::A)) {
            self.play_selected_item(lib, false);
        }
        if ctx.input(|i| i.key_pressed(Key::N)) {
            let _ = self.player_tx.send(PlayerCommand::Skip);
        }

        // --- NAVIGATIE PIJLTJES ---
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            match self.current_level {
                NavLevel::Artist => {
                    if self.selected_artist + 1 < lib.artists.len() {
                        self.selected_artist += 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    let albums = &lib.artists[self.selected_artist].albums;
                    if self.selected_album + 1 < albums.len() {
                        self.selected_album += 1;
                        self.scroll_to_selection = true;
                    } else if self.selected_artist + 1 < lib.artists.len() {
                        self.selected_artist += 1;
                        self.selected_album = 0;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if self.selected_disk + 1 < disks.len() {
                        self.selected_disk += 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Track => {
                    let tracks = &lib.artists[self.selected_artist].albums[self.selected_album]
                        .disks[self.selected_disk]
                        .tracks;
                    if self.selected_track + 1 < tracks.len() {
                        self.selected_track += 1;
                        self.scroll_to_selection = true;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            match self.current_level {
                NavLevel::Artist => {
                    if self.selected_artist > 0 {
                        self.selected_artist -= 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    if self.selected_album > 0 {
                        self.selected_album -= 1;
                        self.scroll_to_selection = true;
                    } else if self.selected_artist > 0 {
                        self.selected_artist -= 1;
                        self.selected_album = lib.artists[self.selected_artist]
                            .albums
                            .len()
                            .saturating_sub(1);
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    if self.selected_disk > 0 {
                        self.selected_disk -= 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Track => {
                    if self.selected_track > 0 {
                        self.selected_track -= 1;
                        self.scroll_to_selection = true;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            match self.current_level {
                NavLevel::Artist => {
                    if !lib.artists[self.selected_artist].albums.is_empty() {
                        self.current_level = NavLevel::Album;
                        self.selected_album = 0;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if !disks.is_empty() {
                        if disks.len() == 1 && disks[0].name == "Default" {
                            self.current_level = NavLevel::Track;
                            self.selected_disk = 0;
                            self.selected_track = 0;
                        } else {
                            self.current_level = NavLevel::Disk;
                            self.selected_disk = 0;
                        }
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    if !lib.artists[self.selected_artist].albums[self.selected_album].disks
                        [self.selected_disk]
                        .tracks
                        .is_empty()
                    {
                        self.current_level = NavLevel::Track;
                        self.selected_track = 0;
                        self.scroll_to_selection = true;
                    }
                }
                _ => {}
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            match self.current_level {
                NavLevel::Album => {
                    self.current_level = NavLevel::Artist;
                    self.scroll_to_selection = true;
                }
                NavLevel::Disk => {
                    self.current_level = NavLevel::Album;
                    self.scroll_to_selection = true;
                }
                NavLevel::Track => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if disks.len() == 1 && disks[0].name == "Default" {
                        self.current_level = NavLevel::Album;
                    } else {
                        self.current_level = NavLevel::Disk;
                    }
                    self.scroll_to_selection = true;
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for MusicPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update Library Status
        while let Ok(msg) = self.scanner_rx.try_recv() {
            if let ScannerMessage::LibraryLoaded(lib) = msg {
                self.library = Some(lib);
                // Als we aan het zoeken waren, herbereken de filter op de nieuwe library
                if !self.search_query.is_empty() {
                    self.filtered_library = Some(filter_library(
                        self.library.as_ref().unwrap(),
                        &self.search_query,
                    ));
                }
            }
        }

        // Update Now Playing Status
        while let Ok(event) = self.player_event_rx.try_recv() {
            match event {
                PlayerEvent::NowPlaying(path) => {
                    if let Some(file_name) = Path::new(&path).file_name() {
                        self.now_playing = Some(file_name.to_string_lossy().into_owned());
                    }
                }
                PlayerEvent::Stopped => self.now_playing = None,
            }
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.is_search_active = true;
            self.search_query.clear();
            self.filtered_library = None; // Reset bij nieuw zoeken
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
        }
        self.handle_keyboard_navigation(ctx);

        // if !ctx.wants_keyboard_input()
        //     && ctx.input(|i| i.key_pressed(egui::Key::Slash))
        //     && !self.is_search_active
        // {
        //     self.is_search_active = true;
        //     self.search_query.clear();
        //     self.search_results.clear();
        //     self.selected_search_index = 0;
        // }
        // --- HELP SCHERM TEKENEN ---
        if self.show_help {
            egui::Window::new("Sneltoetsen & Help")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Toetsenbord Navigatie").strong());
                    ui.label("• Pijltjestoetsen : Navigeer door de bibliotheek");
                    ui.label("• T : Wissel weergave (Lijst / Covers)");
                    ui.add_space(5.0);
                    ui.label(RichText::new("Muziek Besturing").strong());
                    ui.label("• Enter : Speel selectie af (wist wachtrij)");
                    ui.label("• Spatie : Pauzeer / Hervat");
                    ui.label("• A : Voeg selectie toe achteraan de wachtrij");
                    ui.label("• N : Skip naar het volgende nummer");
                    ui.add_space(5.0);
                    ui.label(RichText::new("Extra").strong());
                    ui.label("• R : Selecteer een willekeurig album");
                    ui.label("• F5 : Forceer een rescan van de bibliotheek");
                    ui.label("• ? of H : Toon / verberg dit helpvenster");
                    ui.separator();
                    if ui.button("Sluiten").clicked() {
                        self.show_help = false;
                    }
                });
        }

        // Check of de initiële scan klaar is
        if self.library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Bibliotheek scannen...").size(24.0));
                });
            });
            ctx.request_repaint();
            return;
        }

        // --- NOW PLAYING BALK ONDERAAN ---
        if let Some(track) = &self.now_playing {
            egui::TopBottomPanel::bottom("now_playing_panel").show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🎵 Nu aan het spelen:")
                            .color(Color32::from_rgb(100, 200, 100))
                            .strong(),
                    );
                    ui.label(track);
                });
                ui.add_space(8.0);
            });
        }

        // --- ZOEKBALK RENDEREN ---
        if self.is_search_active {
            egui::Window::new("Zoeken in Bibliotheek")
                .collapsible(false)
                .resizable(false)
                .default_width(600.0)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                .show(ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Typ om te zoeken... (Esc om te sluiten)")
                            .id(self.search_input_id)
                            .desired_width(f32::INFINITY),
                    );

                    // --- DE CURSOR FIX ---
                    // Forceer focus direct nadat het veld is getekend als de zoekbalk actief is
                    if self.is_search_active && !response.has_focus() {
                        ctx.memory_mut(|m| m.request_focus(self.search_input_id));
                    }

                    if response.changed() {
                        self.current_level = NavLevel::Artist;
                        self.selected_artist = 0;
                        self.selected_album = 0;
                        self.selected_disk = 0;
                        self.selected_track = 0;

                        if let Some(full_lib) = &self.library {
                            if self.search_query.trim().is_empty() {
                                self.filtered_library = None;
                            } else {
                                self.filtered_library =
                                    Some(filter_library(full_lib, &self.search_query));
                            }
                        }
                    }
                    if response.has_focus() {
                        // ESC: Annuleer, wis filter, sluit venster
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.is_search_active = false;
                            self.filtered_library = None; // Reset de filter!
                            self.search_query.clear();
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }

                        // ENTER: Sluit zoekvenster, maar BEHOUD de filter!
                        // De focus wordt vrijgegeven, waardoor je nu met de pijltjestoetsen
                        // door de gefilterde resultaten in het hoofdscherm kunt navigeren.
                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.is_search_active = false;
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }
                    }

                    // egui::ScrollArea::vertical()
                    //     .max_height(300.0)
                    //     .show(ui, |ui| {
                    //         if self.search_results.is_empty() && !self.search_query.is_empty() {
                    //             ui.label("Geen resultaten gevonden.");
                    //         }

                    //         for (index, result) in self.search_results.iter().enumerate() {
                    //             let is_selected = index == self.selected_search_index;
                    //             let bg_color = if is_selected {
                    //                 ui.visuals().selection.bg_fill
                    //             } else {
                    //                 ui.visuals().widgets.inactive.weak_bg_fill
                    //             };

                    //             ui.horizontal(|ui| {
                    //                 ui.painter().rect_filled(
                    //                     ui.available_rect_before_wrap(),
                    //                     egui::Rounding::same(4.0),
                    //                     bg_color,
                    //                 );

                    //                 let text_color = if is_selected {
                    //                     ui.visuals().selection.stroke.color
                    //                 } else {
                    //                     ui.visuals().text_color()
                    //                 };

                    //                 ui.label(
                    //                     egui::RichText::new(&result.track.title).color(text_color),
                    //                 );
                    //                 ui.label(
                    //                     egui::RichText::new(format!(
                    //                         " - {} • {}",
                    //                         result.artist_name, result.album_title
                    //                     ))
                    //                     .color(text_color)
                    //                     .size(12.0),
                    //                 );
                    //             });

                    //             if ui
                    //                 .interact(
                    //                     ui.available_rect_before_wrap(),
                    //                     ui.id().with(index),
                    //                     egui::Sense::click(),
                    //                 )
                    //                 .clicked()
                    //             {
                    //                 let _ = self.player_tx.send(
                    //                     crate::player::PlayerCommand::ReplaceQueue(vec![result
                    //                         .track
                    //                         .path
                    //                         .clone()]),
                    //                 );
                    //                 self.is_search_active = false;
                    //             }
                    //         }
                    //     });
                });
        }

        // --- HOOFDSCHERM ---
        let current_lib = self.filtered_library.as_ref().or(self.library.as_ref());
        let Some(current_lib) = current_lib else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Bibliotheek scannen...").size(24.0));
                });
            });
            ctx.request_repaint();
            return;
        };
        // --- CHECK: Lege zoekresultaten? Toon de afbeelding ---
        if current_lib.artists.is_empty() && self.filtered_library.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Geen resultaten gevonden")
                            .size(28.0)
                            .color(egui::Color32::YELLOW),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("voor: \"{}\"", self.search_query))
                            .size(16.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(30.0);

                    // Laad de afbeelding - kies EEN van de twee opties hieronder:

                    // OPTIE A: Embedded (aanbevolen - geen externe bestanden nodig)
                    let image_bytes = include_bytes!("../assets/no_results.png");
                    ui.add(
                        egui::Image::from_bytes("bytes://no_results.png", image_bytes.as_ref())
                            .max_width(600.0)
                            .max_height(600.0),
                    );

                    // OPTIE B: Extern bestand (makkelijker om te vervangen zonder hercompileren)
                    // let image_path = "assets/no_results.png";
                    // if std::path::Path::new(image_path).exists() {
                    //     ui.add(
                    //         egui::Image::new(format!("file://{}", image_path))
                    //             .max_width(400.0)
                    //             .max_height(400.0),
                    //     );
                    // }

                    ui.add_space(30.0);
                    ui.label(
                        egui::RichText::new("Druk op Esc om terug te gaan")
                            .size(14.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
            ctx.request_repaint();
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Visuele indicator dat we aan het zoeken zijn
                // Visuele indicator dat we aan het zoeken zijn + HIT COUNTER
                if self.filtered_library.is_some() {
                    // Tel het totale aantal tracks in de gefilterde bibliotheek
                    let hit_count: usize = current_lib
                        .artists
                        .iter()
                        .map(|a| {
                            a.albums
                                .iter()
                                .map(|al| al.disks.iter().map(|d| d.tracks.len()).sum::<usize>())
                                .sum::<usize>()
                        })
                        .sum::<usize>();

                    ui.label(
                        egui::RichText::new(format!("🔍 {} resultaten voor: ", hit_count))
                            .color(egui::Color32::YELLOW),
                    );
                    ui.label(egui::RichText::new(&self.search_query).strong());
                    ui.label(
                        egui::RichText::new(" (Esc om te wissen) ")
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                } else {
                    ui.label(egui::RichText::new("Bibliotheek").color(egui::Color32::GRAY));
                }

                // Breadcrumb navigatie (aangepast om met current_lib te werken)
                if let Some(artist) = current_lib.artists.get(self.selected_artist) {
                    ui.label(format!(" > {}", artist.name));
                    if matches!(
                        self.current_level,
                        NavLevel::Album | NavLevel::Disk | NavLevel::Track
                    ) {
                        if let Some(album) = artist.albums.get(self.selected_album) {
                            ui.label(format!(" > {}", album.title));
                            if matches!(self.current_level, NavLevel::Disk | NavLevel::Track) {
                                if let Some(disk) = album.disks.get(self.selected_disk) {
                                    ui.label(format!(" > {}", disk.name));
                                }
                            }
                        }
                    }
                }
            });

            ui.separator();

            match self.view_mode {
                ViewMode::AlbumCover if self.current_level != NavLevel::Track => {
                    if self.current_level == NavLevel::Artist {
                        let albums = &current_lib.artists[self.selected_artist].albums;
                        let num_albums = albums.len();

                        if num_albums == 0 {
                            ui.centered_and_justified(|ui| {
                                ui.label("Geen albums");
                            });
                        } else {
                            ScrollArea::vertical().show(ui, |ui| {
                                let available = ui.available_width();
                                let desired_thumb = 220.0_f32;
                                let mut columns = (available / desired_thumb).floor() as usize;
                                if columns == 0 {
                                    columns = 1;
                                }
                                columns = std::cmp::min(columns, std::cmp::max(1, num_albums));
                                let padding = 12.0_f32;
                                let thumb_w = ((available - padding * (columns as f32 + 1.0))
                                    / columns as f32)
                                    .max(80.0)
                                    .min(600.0);
                                let thumb_size = egui::vec2(thumb_w, thumb_w);

                                if num_albums == 1 {
                                    ui.centered_and_justified(|ui| {
                                        if let Some(path) = &albums[0].cover_path {
                                            let big_w = (available * 0.6).max(200.0).min(800.0);
                                            let resp = ui.add_sized(
                                                egui::vec2(big_w, big_w),
                                                Image::new(format!("file://{}", path))
                                                    .show_loading_spinner(false)
                                                    .sense(egui::Sense::click()),
                                            );
                                            if resp.clicked() {
                                                self.current_level = NavLevel::Album;
                                                self.selected_album = 0;
                                                self.scroll_to_selection = true;
                                            }
                                        }
                                        ui.add_space(6.0);
                                        ui.label(RichText::new(&albums[0].title).size(20.0));
                                    });
                                } else {
                                    ui.columns(columns, |cols| {
                                        for (i, album) in albums.iter().enumerate() {
                                            let col = &mut cols[i % columns];

                                            // FIX: Forceer een centrering layout binnen de kolom.
                                            // Dit zorgt ervoor dat zowel de cover als de tekst netjes onder elkaar in het midden van de kolom staan.
                                            col.with_layout(
                                                egui::Layout::top_down(egui::Align::Center),
                                                |col_ui| {
                                                    if let Some(path) = &album.cover_path {
                                                        let resp = col_ui.add_sized(
                                                            thumb_size,
                                                            Image::new(format!("file://{}", path))
                                                                .show_loading_spinner(false)
                                                                .sense(egui::Sense::click()),
                                                        );
                                                        if resp.clicked() {
                                                            self.current_level = NavLevel::Album;
                                                            self.selected_album = i;
                                                            self.scroll_to_selection = true;
                                                        }
                                                    } else {
                                                        // Ruimte reserveren als er geen cover is, zodat de tekst niet naar boven schuift
                                                        col_ui.add_space(thumb_size.y);
                                                    }

                                                    col_ui.add_space(6.0);
                                                    // De tekst wordt nu netjes gecentreerd onder de cover getoond
                                                    col_ui.label(
                                                        RichText::new(&album.title)
                                                            .size(14.0)
                                                            .color(Color32::WHITE), // Ook direct de tekst wit gemaakt voor beter contrast
                                                    );
                                                },
                                            );
                                        }
                                    });
                                }
                            });
                        }
                    } else {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.centered_and_justified(|ui| {
                                if let Some(album) = current_lib
                                    .artists
                                    .get(self.selected_artist)
                                    .and_then(|a| a.albums.get(self.selected_album))
                                {
                                    if let Some(path) = &album.cover_path {
                                        let available = ui.available_width();
                                        let size_w = (available * 0.5).max(200.0).min(1200.0);
                                        let _ = ui.add_sized(
                                            egui::vec2(size_w, size_w),
                                            Image::new(format!("file://{}", path))
                                                .show_loading_spinner(false),
                                        );
                                    }
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(&album.title).size(20.0));
                                }
                            });
                        });
                    }
                }
                _ => {
                    ScrollArea::vertical().show(ui, |ui| match self.current_level {
                        NavLevel::Artist => {
                            for (i, artist) in current_lib.artists.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let resp = ui.selectable_label(
                                                i == self.selected_artist,
                                                RichText::new(&artist.name).size(18.0),
                                            );
                                            if resp.clicked() {
                                                self.selected_artist = i;
                                                self.scroll_to_selection = true;
                                            }
                                            if i == self.selected_artist && self.scroll_to_selection
                                            {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                        }
                        NavLevel::Album => {
                            for (i, album) in current_lib.artists[self.selected_artist]
                                .albums
                                .iter()
                                .enumerate()
                            {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let resp = ui.selectable_label(
                                                i == self.selected_album,
                                                RichText::new(&album.title).size(18.0),
                                            );
                                            if resp.clicked() {
                                                self.selected_album = i;
                                                self.scroll_to_selection = true;
                                            }
                                            if i == self.selected_album && self.scroll_to_selection
                                            {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                        }
                        NavLevel::Disk => {
                            for (i, disk) in current_lib.artists[self.selected_artist].albums
                                [self.selected_album]
                                .disks
                                .iter()
                                .enumerate()
                            {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let resp = ui.selectable_label(
                                                i == self.selected_disk,
                                                RichText::new(format!("CD: {}", disk.name))
                                                    .size(16.0),
                                            );
                                            if resp.clicked() {
                                                self.selected_disk = i;
                                                self.scroll_to_selection = true;
                                            }
                                            if i == self.selected_disk && self.scroll_to_selection {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                        }
                        NavLevel::Track => {
                            for (i, track) in current_lib.artists[self.selected_artist].albums
                                [self.selected_album]
                                .disks[self.selected_disk]
                                .tracks
                                .iter()
                                .enumerate()
                            {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let resp = ui.selectable_label(
                                                i == self.selected_track,
                                                RichText::new(&track.title).size(16.0),
                                            );
                                            if resp.clicked() {
                                                self.selected_track = i;
                                                self.scroll_to_selection = true;
                                            }
                                            if i == self.selected_track && self.scroll_to_selection
                                            {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                        }
                    });
                } // _ => {
                  //     ScrollArea::vertical().show(ui, |ui| match self.current_level {
                  //         NavLevel::Artist => {
                  //             // Wrap the loop in a centered layout
                  //             ui.with_layout(
                  //                 egui::Layout::centered_and_justified(egui::Direction::TopDown),
                  //                 |ui| {
                  //                     for (i, artist) in current_lib.artists.iter().enumerate() {
                  //                         let resp = ui.selectable_label(
                  //                             i == self.selected_artist,
                  //                             RichText::new(&artist.name).size(18.0),
                  //                         );
                  //                         if resp.clicked() {
                  //                             self.selected_artist = i;
                  //                             self.scroll_to_selection = true;
                  //                         }
                  //                         if i == self.selected_artist && self.scroll_to_selection {
                  //                             resp.scroll_to_me(None);
                  //                         }
                  //                     }
                  //                 },
                  //             );
                  //         }
                  //         NavLevel::Album => {
                  //             ui.with_layout(
                  //                 egui::Layout::centered_and_justified(egui::Direction::TopDown),
                  //                 |ui| {
                  //                     for (i, album) in current_lib.artists[self.selected_artist]
                  //                         .albums
                  //                         .iter()
                  //                         .enumerate()
                  //                     {
                  //                         let resp = ui.selectable_label(
                  //                             i == self.selected_album,
                  //                             RichText::new(&album.title).size(18.0),
                  //                         );
                  //                         if resp.clicked() {
                  //                             self.selected_album = i;
                  //                             self.scroll_to_selection = true;
                  //                         }
                  //                         if i == self.selected_album && self.scroll_to_selection {
                  //                             resp.scroll_to_me(None);
                  //                         }
                  //                     }
                  //                 },
                  //             );
                  //         }
                  //         NavLevel::Disk => {
                  //             ui.with_layout(
                  //                 egui::Layout::centered_and_justified(egui::Direction::TopDown),
                  //                 |ui| {
                  //                     for (i, disk) in current_lib.artists[self.selected_artist]
                  //                         .albums[self.selected_album]
                  //                         .disks
                  //                         .iter()
                  //                         .enumerate()
                  //                     {
                  //                         let resp = ui.selectable_label(
                  //                             i == self.selected_disk,
                  //                             RichText::new(format!("CD: {}", disk.name)).size(16.0),
                  //                         );
                  //                         if resp.clicked() {
                  //                             self.selected_disk = i;
                  //                             self.scroll_to_selection = true;
                  //                         }
                  //                         if i == self.selected_disk && self.scroll_to_selection {
                  //                             resp.scroll_to_me(None);
                  //                         }
                  //                     }
                  //                 },
                  //             );
                  //         }
                  //         NavLevel::Track => {
                  //             ui.with_layout(
                  //                 egui::Layout::centered_and_justified(egui::Direction::TopDown),
                  //                 |ui| {
                  //                     for (i, track) in current_lib.artists[self.selected_artist]
                  //                         .albums[self.selected_album]
                  //                         .disks[self.selected_disk]
                  //                         .tracks
                  //                         .iter()
                  //                         .enumerate()
                  //                     {
                  //                         let resp = ui.selectable_label(
                  //                             i == self.selected_track,
                  //                             RichText::new(&track.title).size(16.0),
                  //                         );
                  //                         if resp.clicked() {
                  //                             self.selected_track = i;
                  //                             self.scroll_to_selection = true;
                  //                         }
                  //                         if i == self.selected_track && self.scroll_to_selection {
                  //                             resp.scroll_to_me(None);
                  //                         }
                  //                     }
                  //                 },
                  //             );
                  //         }
                  //     });
                  // }

                  // _ => {
                  //     ScrollArea::vertical().show(ui, |ui| match self.current_level {
                  //         NavLevel::Artist => {
                  //             for (i, artist) in current_lib.artists.iter().enumerate() {
                  //                 let resp = ui.selectable_label(
                  //                     i == self.selected_artist,
                  //                     RichText::new(&artist.name).size(18.0),
                  //                 );
                  //                 if resp.clicked() {
                  //                     self.selected_artist = i;
                  //                     self.scroll_to_selection = true;
                  //                 }
                  //                 if i == self.selected_artist && self.scroll_to_selection {
                  //                     resp.scroll_to_me(None);
                  //                 }
                  //             }
                  //         }
                  //         NavLevel::Album => {
                  //             for (i, album) in current_lib.artists[self.selected_artist]
                  //                 .albums
                  //                 .iter()
                  //                 .enumerate()
                  //             {
                  //                 let resp = ui.selectable_label(
                  //                     i == self.selected_album,
                  //                     RichText::new(&album.title).size(18.0),
                  //                 );
                  //                 if resp.clicked() {
                  //                     self.selected_album = i;
                  //                     self.scroll_to_selection = true;
                  //                 }
                  //                 if i == self.selected_album && self.scroll_to_selection {
                  //                     resp.scroll_to_me(None);
                  //                 }
                  //             }
                  //         }
                  //         NavLevel::Disk => {
                  //             for (i, disk) in current_lib.artists[self.selected_artist].albums
                  //                 [self.selected_album]
                  //                 .disks
                  //                 .iter()
                  //                 .enumerate()
                  //             {
                  //                 let resp = ui.selectable_label(
                  //                     i == self.selected_disk,
                  //                     RichText::new(format!("CD: {}", disk.name)).size(16.0),
                  //                 );
                  //                 if resp.clicked() {
                  //                     self.selected_disk = i;
                  //                     self.scroll_to_selection = true;
                  //                 }
                  //                 if i == self.selected_disk && self.scroll_to_selection {
                  //                     resp.scroll_to_me(None);
                  //                 }
                  //             }
                  //         }
                  //         NavLevel::Track => {
                  //             for (i, track) in current_lib.artists[self.selected_artist].albums
                  //                 [self.selected_album]
                  //                 .disks[self.selected_disk]
                  //                 .tracks
                  //                 .iter()
                  //                 .enumerate()
                  //             {
                  //                 // Center each track item
                  //                 ui.horizontal(|ui| {
                  //                     ui.with_layout(
                  //                         egui::Layout::centered_and_justified(
                  //                             egui::Direction::TopDown,
                  //                         ),
                  //                         |ui| {
                  //                             let resp = ui.selectable_label(
                  //                                 i == self.selected_track,
                  //                                 RichText::new(&track.title).size(16.0),
                  //                             );
                  //                             if resp.clicked() {
                  //                                 self.selected_track = i;
                  //                                 self.scroll_to_selection = true;
                  //                             }
                  //                             if i == self.selected_track && self.scroll_to_selection
                  //                             {
                  //                                 resp.scroll_to_me(None);
                  //                             }
                  //                         },
                  //                     );
                  //                 });
                  //             }
                  //         }
                  //     });
                  // }
            }
        });

        self.scroll_to_selection = false;
        ctx.request_repaint();
    }
}
