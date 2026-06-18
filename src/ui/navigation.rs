use crate::player::PlayerCommand;
use crate::ui::shortcuts;
use crate::ui::types::{FilterNode, NavLevel, ViewMode};
use eframe::egui;
use std::fmt::Write;
use std::path::Path;

use super::app::MusicPlayerApp;

impl MusicPlayerApp {
    pub fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let cfg = self.config.shortcuts.clone();

        // --- ESCAPE ---
        if shortcuts::check_action(&cfg, ctx, "Escape") {
            self.selected_tracks.clear();

            // 1. Eerst picker wissen (Gooi de openstaande picker echt uit het pad)
            if self.is_picker_active() {
                self.filter_path.remove(self.filter_step); // Verwijder de "None" node

                // Herbereken zonder deze picker
                self.recompute();
                self.populate_current_picker();

                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.scroll_to_selection = true;
                return;
            }

            // 2. Dan search wissen
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

            // 3. Dan filterpath wissen
            if !self.filter_path.is_empty() {
                self.reset_filters();
                return;
            }
        }

        // --- F5: RESCAN ---
        if shortcuts::check_action(&cfg, ctx, "Rescan") {
            let _ = std::fs::remove_file("library_cache.bin");
            self.library = None;
            self.filtered_library = None;
            self.cached_filtered = None;
            self.filter_path.clear();
            self.filter_step = 0;
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

        // --- F6: RECONNECT AUDIO ---
        if shortcuts::check_action(&cfg, ctx, "ReconnectAudio") {
            let _ = self.player_tx.send(PlayerCommand::ReconnectAudio);
            self.now_playing = Some("Audio verbinding herstellen...".to_string());
        }

        // --- HELP ---
        if shortcuts::check_action(&cfg, ctx, "Help") {
            self.show_help = !self.show_help;
        }

        // --- G: GENRE PICKER ---
        if shortcuts::check_action(&cfg, ctx, "GenreBrowse") {
            self.toggle_genre_picker();
            return;
        }

        // --- Y: YEAR PICKER ---
        if shortcuts::check_action(&cfg, ctx, "YearBrowse") {
            self.toggle_year_picker();
            return;
        }

        // --- C: COMPOSER PICKER ---
        if shortcuts::check_action(&cfg, ctx, "ComposerBrowse") {
            self.toggle_composer_picker();
            return;
        }

        // --- S: SORT TOGGLE ---
        if shortcuts::check_action(&cfg, ctx, "SortToggle") {
            self.toggle_sort();
            return;
        }

        // --- B: RECENT ALBUMS ---
        if shortcuts::check_action(&cfg, ctx, "RecentAlbums") {
            // Toggle: bij tweede B terug naar bibliotheek
            if !self.recent_albums.is_empty() {
                self.recent_albums.clear();
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.scroll_to_selection = true;
            } else {
                self.enter_recent_mode();
            }
            return;
        }

        // --- Z: SELECTION BROWSE ---
        if shortcuts::check_action(&cfg, ctx, "SelectionBrowse") {
            if self.selected_tracks.is_empty() {
                return;
            }
            // Gebruik selection-filter: maak een gefilterde library van alleen geselecteerde tracks
            if let Some(lib) = &self.cached_filtered {
                let selection_lib =
                    MusicPlayerApp::build_selection_library(lib, &self.selected_tracks);
                if !selection_lib.artists.is_empty() {
                    self.filtered_library = Some(selection_lib);
                    self.current_level = NavLevel::Artist;
                    self.selected_artist = 0;
                    self.selected_album = 0;
                    self.selected_disk = 0;
                    self.selected_track = 0;
                    self.scroll_to_selection = true;
                }
            }
            return;
        }

        // --- Recent albums navigation ---
        if !self.recent_albums.is_empty() {
            if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                if self.selected_recent + 1 < self.recent_albums.len() {
                    self.selected_recent += 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                if self.selected_recent > 0 {
                    self.selected_recent -= 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(&cfg, ctx, "Select") {
                if let Some((_, album)) = self.recent_albums.get(self.selected_recent) {
                    let mut queue = Vec::new();
                    for disk in &album.disks {
                        for track in &disk.tracks {
                            queue.push(track.path.clone());
                        }
                    }
                    let _ = self.player_tx.send(PlayerCommand::ReplaceQueue(queue));
                }
            }
            // Escape verlaat recent albums
            if shortcuts::check_action(&cfg, ctx, "Escape") {
                self.recent_albums.clear();
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.scroll_to_selection = true;
            }
            return;
        }
        // --- NAVIGATE LEFT (Binnen een picker) ---
        // Voorkom dat de 'return' in PICKER NAVIGATION de linkerpijl opslokt!
        // B: Navigeer links in een picker: ga een stap terug
        if self.is_picker_active() && shortcuts::check_action(&cfg, ctx, "NavigateLeft") {
            self.step_back_filter();
            return;
        }

        // --- I: TRACK DETAILS ---
        if shortcuts::check_action(&cfg, ctx, "TrackDetails") {
            let has_selection = !self.selected_tracks.is_empty();
            if has_selection || self.current_level == NavLevel::Track {
                self.save_status = None;
                self.raw_tags_display.clear();
                self.read_error = None;
                self.edit_title.clear();
                self.edit_artist.clear();
                self.edit_album.clear();
                self.edit_genre.clear();
                self.edit_year.clear();
                self.edit_composer.clear();
                self.update_title = false;
                self.update_artist = false;
                self.update_album = false;
                self.update_genre = false;
                self.update_year = false;
                self.update_composer = false;

                use lofty::file::TaggedFileExt;
                use lofty::probe::Probe;
                use lofty::tag::{Accessor, ItemKey, ItemValue};

                if self.selected_tracks.is_empty() {
                    let lib_for_path = self.active_library();
                    if let Some(track_path) =
                        lib_for_path.and_then(|lib| self.get_current_track_path(lib))
                    {
                        self.tracks_to_edit = vec![track_path];
                    }
                } else {
                    self.tracks_to_edit = self.selected_tracks.iter().cloned().collect();
                    self.tracks_to_edit.sort();
                }

                if let Some(first_path) = self.tracks_to_edit.first() {
                    self.editing_track_path = Some(first_path.clone());

                    // 2. CORRECTHEID: Gebruik Option::and_then() om een waarde te krijgen of None terug te geven
                    match Probe::open(first_path).and_then(|p| p.read()) {
                        Ok(tagged_file) => {
                            let mut raw_text = String::new();

                            // 1. EFFICIËNTER: Gebruik writeln! i.p.v. push_str(&format!())
                            for tag in tagged_file.tags() {
                                let _ = writeln!(
                                    &mut raw_text,
                                    "--- Tag Type: {:?} ---",
                                    tag.tag_type()
                                );
                                for item in tag.items() {
                                    let _ = writeln!(
                                        &mut raw_text,
                                        "{:?}: {:?}",
                                        item.key(),
                                        item.value()
                                    );
                                }
                            }

                            self.raw_tags_display = if raw_text.is_empty() {
                                "Geen tags gevonden.".to_string()
                            } else {
                                raw_text
                            };

                            // Scan ALLE tags (net als de scanner), niet alleen primary_tag
                            // want Id3v2 kan leeg zijn terwijl Id3v1 de data bevat
                            let mut found_title: Option<String> = None;
                            let mut found_artist: Option<String> = None;
                            let mut found_album: Option<String> = None;
                            let mut found_genres: Vec<String> = Vec::new();
                            let mut found_year: Option<String> = None;
                            let mut found_composer: Option<String> = None;

                            for tag in tagged_file.tags() {
                                // Title/Artist/Album via Accessor (eerste gevonden wint)
                                if found_title.is_none() {
                                    if let Some(t) = tag.title() {
                                        found_title = Some(t.to_string());
                                    }
                                }
                                if found_artist.is_none() {
                                    if let Some(a) = tag.artist() {
                                        found_artist = Some(a.to_string());
                                    }
                                }
                                if found_album.is_none() {
                                    if let Some(a) = tag.album() {
                                        found_album = Some(a.to_string());
                                    }
                                }

                                for item in tag.items() {
                                    match item.key() {
                                        ItemKey::Genre => {
                                            if let ItemValue::Text(text) = item.value() {
                                                found_genres.push(text.clone());
                                            }
                                        }
                                        key if matches!(
                                            key,
                                            ItemKey::Year
                                                | ItemKey::RecordingDate
                                                | ItemKey::OriginalReleaseDate
                                        ) || matches!(key, ItemKey::Unknown(k) if k.to_lowercase() == "originalyear" || k.to_lowercase() == "toryear") =>
                                        {
                                            if found_year.is_none() {
                                                if let ItemValue::Text(text) = item.value() {
                                                    found_year =
                                                        Some(text.chars().take(4).collect());
                                                }
                                            }
                                        }
                                        ItemKey::Composer => {
                                            if found_composer.is_none() {
                                                if let ItemValue::Text(text) = item.value() {
                                                    found_composer = Some(text.clone());
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            self.edit_title = found_title.unwrap_or_default();
                            self.edit_artist = found_artist.unwrap_or_default();
                            self.edit_album = found_album.unwrap_or_default();
                            self.edit_genre = found_genres.join("; ");
                            self.edit_year = found_year.unwrap_or_default();
                            self.edit_composer = found_composer.unwrap_or_default();
                        }
                        Err(e) => {
                            self.read_error = Some(format!("{:?}", e));
                            self.raw_tags_display =
                                "Fout bij het parsen van de audio-container.".to_string();
                        }
                    }
                }
                self.show_track_details = true;
            }
        }

        // --- PICKER NAVIGATION ---
        // Alleen actief als we op een picker staan (None-waarde node)
        if let Some(node) = self.filter_path.get(self.filter_step) {
            match node {
                FilterNode::Genre(_) if self.is_picker_active() => {
                    let len = self.genres.len();
                    if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                        if self.selected_genre + 1 < len {
                            self.selected_genre += 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                        if self.selected_genre > 0 {
                            self.selected_genre -= 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "Select")
                        || shortcuts::check_action(&cfg, ctx, "NavigateRight")
                    {
                        if let Some(genre_name) =
                            self.genres.get(self.selected_genre).map(|(n, _)| n.clone())
                        {
                            self.select_genre(&genre_name);
                        }
                    }
                    // M op genre: alle tracks van dit genre markeren
                    if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
                        if let Some((genre_name, _)) = self.genres.get(self.selected_genre).cloned()
                        {
                            if let Some(ref lib) = self.cached_filtered {
                                let genre_lib = crate::search::filter_by_genre(lib, &genre_name);
                                let paths: Vec<String> = genre_lib
                                    .artists
                                    .iter()
                                    .flat_map(|a| {
                                        a.albums.iter().flat_map(|al| {
                                            al.disks.iter().flat_map(|d| {
                                                d.tracks.iter().map(|t| t.path.clone())
                                            })
                                        })
                                    })
                                    .collect();
                                if !paths.is_empty() {
                                    let all_selected = paths
                                        .iter()
                                        .all(|p| self.selected_tracks.contains(p.as_str()));
                                    if all_selected {
                                        for p in &paths {
                                            self.selected_tracks.remove(p);
                                        }
                                    } else {
                                        for p in &paths {
                                            self.selected_tracks.insert(p.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                FilterNode::Year(_) if self.is_picker_active() => {
                    let len = self.years.len();
                    if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                        if self.selected_year + 1 < len {
                            self.selected_year += 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                        if self.selected_year > 0 {
                            self.selected_year -= 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "Select")
                        || shortcuts::check_action(&cfg, ctx, "NavigateRight")
                    {
                        if let Some((year_opt, _)) = self.years.get(self.selected_year) {
                            self.select_year(year_opt.unwrap_or(0));
                        }
                    }
                    // M op jaar: alle tracks van dit jaar markeren
                    if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
                        if let Some((year_opt, _)) = self.years.get(self.selected_year).cloned() {
                            if let Some(ref lib) = self.cached_filtered {
                                let year_lib =
                                    crate::search::filter_by_year(lib, year_opt.unwrap_or(0));
                                let paths: Vec<String> = year_lib
                                    .artists
                                    .iter()
                                    .flat_map(|a| {
                                        a.albums.iter().flat_map(|al| {
                                            al.disks.iter().flat_map(|d| {
                                                d.tracks.iter().map(|t| t.path.clone())
                                            })
                                        })
                                    })
                                    .collect();
                                if !paths.is_empty() {
                                    let all_selected = paths
                                        .iter()
                                        .all(|p| self.selected_tracks.contains(p.as_str()));
                                    if all_selected {
                                        for p in &paths {
                                            self.selected_tracks.remove(p);
                                        }
                                    } else {
                                        for p in &paths {
                                            self.selected_tracks.insert(p.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                FilterNode::Composer(_) if self.is_picker_active() => {
                    let len = self.composers.len();
                    if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                        if self.selected_composer + 1 < len {
                            self.selected_composer += 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                        if self.selected_composer > 0 {
                            self.selected_composer -= 1;
                            self.scroll_to_selection = true;
                        }
                    }
                    if shortcuts::check_action(&cfg, ctx, "Select")
                        || shortcuts::check_action(&cfg, ctx, "NavigateRight")
                    {
                        if let Some((name, _)) = self.composers.get(self.selected_composer).cloned()
                        {
                            self.select_composer(&name);
                        }
                    }
                    // M op componist: alle tracks van dit componist markeren
                    if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
                        if let Some((name, _)) = self.composers.get(self.selected_composer).cloned()
                        {
                            if let Some(ref lib) = self.cached_filtered {
                                let comp_lib = crate::search::filter_by_composer(lib, &name);
                                let paths: Vec<String> = comp_lib
                                    .artists
                                    .iter()
                                    .flat_map(|a| {
                                        a.albums.iter().flat_map(|al| {
                                            al.disks.iter().flat_map(|d| {
                                                d.tracks.iter().map(|t| t.path.clone())
                                            })
                                        })
                                    })
                                    .collect();
                                if !paths.is_empty() {
                                    let all_selected = paths
                                        .iter()
                                        .all(|p| self.selected_tracks.contains(p.as_str()));
                                    if all_selected {
                                        for p in &paths {
                                            self.selected_tracks.remove(p);
                                        }
                                    } else {
                                        for p in &paths {
                                            self.selected_tracks.insert(p.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        // Kies de actieve library voor navigatie (disjoint borrow, geen clone!)
        let Some(lib) = self
            .filtered_library
            .as_ref()
            .or(self.cached_filtered.as_ref())
            .or(self.library.as_ref())
        else {
            return;
        };

        // --- R: RANDOM ALBUM ---
        if shortcuts::check_action(&cfg, ctx, "RandomAlbum") {
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

        // --- T: TOGGLE VIEW ---
        if shortcuts::check_action(&cfg, ctx, "ToggleView") {
            self.view_mode = match self.view_mode {
                ViewMode::Tracklist => ViewMode::AlbumCover,
                ViewMode::AlbumCover => ViewMode::Tracklist,
            };
        }

        // --- PLAYBACK CONTROLES ---
        if shortcuts::check_action(&cfg, ctx, "PlayPause") {
            let _ = self.player_tx.send(PlayerCommand::PlayPause);
        }
        if shortcuts::check_action(&cfg, ctx, "Select") {
            self.play_selected_item(&lib, true);
        }
        if shortcuts::check_action(&cfg, ctx, "AppendQueue") {
            self.play_selected_item(&lib, false);
        }
        if shortcuts::check_action(&cfg, ctx, "Skip") {
            let _ = self.player_tx.send(PlayerCommand::Skip);
        }
        if shortcuts::check_action(&cfg, ctx, "Rewind") {
            let _ = self.player_tx.send(PlayerCommand::Rewind);
        }
        if shortcuts::check_action(&cfg, ctx, "Forward") {
            let _ = self.player_tx.send(PlayerCommand::Forward);
        }
        if shortcuts::check_action(&cfg, ctx, "RepeatToggle") {
            let _ = self.player_tx.send(PlayerCommand::ToggleRepeat);
        }
        if shortcuts::check_action(&cfg, ctx, "ShuffleToggle") {
            let _ = self.player_tx.send(PlayerCommand::ToggleShuffle);
        }
        if shortcuts::check_action(&cfg, ctx, "VolumeUp") {
            self.volume = (self.volume + 0.1).min(2.0);
            let _ = self.player_tx.send(PlayerCommand::SetVolume(self.volume));
        }
        if shortcuts::check_action(&cfg, ctx, "VolumeDown") {
            self.volume = (self.volume - 0.1).max(0.0);
            let _ = self.player_tx.send(PlayerCommand::SetVolume(self.volume));
        }

        // --- F2: NOW PLAYING NAVIGATIE ---
        if shortcuts::check_action(&cfg, ctx, "NowPlaying") {
            if let Some(ref target) = self.now_playing_path {
                if let Some(ref lib) = self.library {
                    for (ai, artist) in lib.artists.iter().enumerate() {
                        for (ali, album) in artist.albums.iter().enumerate() {
                            for (di, disk) in album.disks.iter().enumerate() {
                                for (ti, track) in disk.tracks.iter().enumerate() {
                                    if track.path == *target {
                                        self.selected_artist = ai;
                                        self.selected_album = ali;
                                        self.selected_disk = di;
                                        self.selected_track = ti;
                                        self.current_level = NavLevel::Track;
                                        self.scroll_to_selection = true;
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- O: OPEN FOLDER ---
        if shortcuts::check_action(&cfg, ctx, "OpenFolder") {
            if let Some(track_path) = self.get_current_track_path(&lib) {
                if let Some(parent) = Path::new(&track_path).parent() {
                    let _ = std::process::Command::new("explorer").arg(parent).spawn();
                }
            }
        }

        // --- M: MARK / UNMARK OP HUIDIG NIVEAU ---
        if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
            let tracks = self
                .filtered_library
                .as_ref()
                .or(self.cached_filtered.as_ref())
                .or(self.library.as_ref())
                .map(|l| self.get_tracks_at_level(l, &self.current_level))
                .unwrap_or_default();
            if !tracks.is_empty() {
                let all_selected = tracks
                    .iter()
                    .all(|p| self.selected_tracks.contains(p.as_str()));
                if all_selected {
                    for p in &tracks {
                        self.selected_tracks.remove(p);
                    }
                } else {
                    for p in &tracks {
                        self.selected_tracks.insert(p.clone());
                    }
                }
            }
        }
        // --- SHIFT+M: CLEAR MARKS ---
        if shortcuts::check_action(&cfg, ctx, "ClearMarks") {
            self.selected_tracks.clear();
            self.tracks_to_edit.clear();
        }

        // --- ARROW NAVIGATION ---
        if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
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
        if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
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
        if shortcuts::check_action(&cfg, ctx, "NavigateRight") {
            if self.current_level == NavLevel::Artist {
                // Als we nog niet door alle pickers heen zijn, ga naar de volgende picker
                // Geen filters (meer): ga naar Album niveau
                if !lib.artists[self.selected_artist].albums.is_empty() {
                    self.current_level = NavLevel::Album;
                    self.selected_album = 0;
                    self.scroll_to_selection = true;
                }
            } else {
                // Normale hiërarchie navigatie (Album -> Disk -> Track)
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
            return;
        }
        if shortcuts::check_action(&cfg, ctx, "NavigateLeft") {
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
                NavLevel::Artist => {
                    // Ga een stap terug in de filter pipeline (en onthoud selectie)
                    self.step_back_filter();
                }
            }
            return;
        }
    }
}
