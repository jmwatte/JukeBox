use super::app::MusicPlayerApp;
use eframe::egui::{self, Color32, RichText, ScrollArea};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use std::path::Path;

impl MusicPlayerApp {
    pub fn save_track_tags(&mut self) {
        if self.tracks_to_edit.is_empty() {
            return;
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for path in &self.tracks_to_edit {
            let result = (|| -> Result<(), String> {
                let tagged_file = Probe::open(path)
                    .map_err(|e| format!("Open: {:?}", e))?
                    .read()
                    .map_err(|e| format!("Read: {:?}", e))?;

                let ext = Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let target_tag_type = match ext.as_str() {
                    "mp3" => TagType::Id3v2,
                    "flac" | "ogg" | "opus" => TagType::VorbisComments,
                    "m4a" | "mp4" => TagType::Mp4Ilst,
                    "wav" => TagType::RiffInfo,
                    _ => TagType::Id3v2,
                };

                let mut tag = Tag::new(target_tag_type);

                let mut existing_title: Option<String> = None;
                let mut existing_artist: Option<String> = None;
                let mut existing_album: Option<String> = None;
                let mut existing_genres: Vec<String> = Vec::new();
                let mut existing_year: Option<String> = None;
                let mut existing_composer: Option<String> = None;

                for existing_tag in tagged_file.tags() {
                    if existing_tag.tag_type() == target_tag_type {
                        tag = existing_tag.clone();
                    }
                    if existing_title.is_none() {
                        existing_title = existing_tag.title().map(|s| s.to_string());
                    }
                    if existing_artist.is_none() {
                        existing_artist = existing_tag.artist().map(|s| s.to_string());
                    }
                    if existing_album.is_none() {
                        existing_album = existing_tag.album().map(|s| s.to_string());
                    }
                    for item in existing_tag.items() {
                        match item.key() {
                            ItemKey::Genre => {
                                if let ItemValue::Text(text) = item.value() {
                                    existing_genres.push(text.clone());
                                }
                            }
                            key if matches!(
                                key,
                                ItemKey::Year
                                    | ItemKey::RecordingDate
                                    | ItemKey::OriginalReleaseDate
                            ) || matches!(key, ItemKey::Unknown(k) if k.to_lowercase() == "originalyear" || k.to_lowercase() == "toryear") =>
                            {
                                if existing_year.is_none() {
                                    if let ItemValue::Text(text) = item.value() {
                                        existing_year = Some(text.chars().take(4).collect());
                                    }
                                }
                            }
                            ItemKey::Composer => {
                                if existing_composer.is_none() {
                                    if let ItemValue::Text(text) = item.value() {
                                        existing_composer = Some(text.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if !self.update_title && tag.title().unwrap_or_default().is_empty() {
                    if let Some(ref t) = existing_title {
                        tag.set_title(t.clone());
                    }
                }
                if !self.update_artist && tag.artist().unwrap_or_default().is_empty() {
                    if let Some(ref a) = existing_artist {
                        tag.set_artist(a.clone());
                    }
                }
                if !self.update_album && tag.album().unwrap_or_default().is_empty() {
                    if let Some(ref a) = existing_album {
                        tag.set_album(a.clone());
                    }
                }
                if !self.update_genre && tag.get(&ItemKey::Genre).is_none() {
                    for g in &existing_genres {
                        tag.insert(TagItem::new(ItemKey::Genre, ItemValue::Text(g.clone())));
                    }
                }
                if !self.update_year && tag.get(&ItemKey::Year).is_none() {
                    if let Some(ref y) = existing_year {
                        tag.insert(TagItem::new(ItemKey::Year, ItemValue::Text(y.clone())));
                    }
                }
                if !self.update_composer && tag.get(&ItemKey::Composer).is_none() {
                    if let Some(ref c) = existing_composer {
                        tag.insert(TagItem::new(ItemKey::Composer, ItemValue::Text(c.clone())));
                    }
                }

                if self.update_title {
                    tag.set_title(self.edit_title.clone());
                }
                if self.update_artist {
                    tag.set_artist(self.edit_artist.clone());
                }
                if self.update_album {
                    tag.set_album(self.edit_album.clone());
                }

                if self.update_genre {
                    tag.remove_key(&ItemKey::Genre);
                    for g in self.edit_genre.split(';') {
                        let trimmed = g.trim();
                        if !trimmed.is_empty() {
                            tag.push(TagItem::new(
                                ItemKey::Genre,
                                ItemValue::Text(trimmed.to_string()),
                            ));
                        }
                    }
                }

                if self.update_year {
                    tag.remove_key(&ItemKey::Year);
                    let trimmed = self.edit_year.trim();
                    if !trimmed.is_empty() {
                        tag.insert(TagItem::new(
                            ItemKey::Year,
                            ItemValue::Text(trimmed.to_string()),
                        ));
                    }
                }

                if self.update_composer {
                    tag.remove_key(&ItemKey::Composer);
                    let trimmed = self.edit_composer.trim();
                    if !trimmed.is_empty() {
                        tag.insert(TagItem::new(
                            ItemKey::Composer,
                            ItemValue::Text(trimmed.to_string()),
                        ));
                    }
                }

                let write_options = WriteOptions::new().remove_others(true);

                tag.save_to_path(path, write_options)
                    .map_err(|e| format!("Save faalde: {:?}", e))?;

                Ok(())
            })();

            match result {
                Ok(_) => {
                    success_count += 1;
                    if let Some(lib) = &mut self.library {
                        for artist in &mut lib.artists {
                            for album in &mut artist.albums {
                                for disk in &mut album.disks {
                                    for track in &mut disk.tracks {
                                        if track.path == *path {
                                            if self.update_title {
                                                track.title = self.edit_title.clone();
                                            }
                                            if self.update_genre {
                                                track.genre = Some(self.edit_genre.clone());
                                            }
                                            if self.update_year {
                                                track.year =
                                                    self.edit_year.trim().parse::<u32>().ok();
                                            }
                                            if self.update_composer {
                                                track.composer = Some(self.edit_composer.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    println!("LOFTY SAVE ERROR voor {:?}: {}", path, e);
                }
            }
        }

        if success_count > 0 && self.tracks_to_edit.len() == 1 {
            if let Some(path) = self.editing_track_path.clone() {
                self.refresh_raw_tags_for_path(&path);
            }
        }

        if error_count == 0 {
            self.save_status = Some(format!(
                "Succesvol opgeslagen in {} bestand(en)!",
                success_count
            ));
        } else {
            self.save_status = Some(format!(
                "{} opgeslagen, {} faalden.",
                success_count, error_count
            ));
        }

        self.selected_tracks.clear();
        self.recompute();
    }

    pub fn show_batch_edit_panel(&mut self, ui: &mut egui::Ui) {
        if self.tracks_to_edit.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Selecteer nummers om tags te bewerken")
                        .color(Color32::GRAY)
                        .italics(),
                );
            });
            return;
        }

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "🎵 Batch Edit: {} bestanden",
                        self.tracks_to_edit.len()
                    ))
                    .strong()
                    .size(14.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("❌ Wis selectie").clicked() {
                        self.selected_tracks.clear();
                        self.tracks_to_edit.clear();
                        self.editing_track_path = None;
                    }
                });
            });
            ui.separator();
            ui.add_space(5.0);

            ui.label(RichText::new("Te schrijven waarden:").strong().size(12.0));

            // Helper die ALLEEN de UI tekent en true teruggeeft als op 📋 is geklikt.
            // Geen self borrow hierbinnen, dus geen borrow checker conflicten!
            let render_field =
                |ui: &mut egui::Ui, label: &str, update: &mut bool, value: &mut String| -> bool {
                    let mut copy_clicked = false;
                    ui.horizontal(|ui| {
                        ui.checkbox(update, "");
                        ui.label(format!("{}:", label));
                        ui.add_sized(
                            [200.0, 20.0],
                            egui::TextEdit::singleline(value).interactive(*update),
                        );
                        if ui
                            .small_button("📋")
                            .on_hover_text("Kopieer van hieronder geselecteerd bestand")
                            .clicked()
                        {
                            copy_clicked = true;
                        }
                    });
                    copy_clicked
                };

            // De mutatie van self gebeurt nu BUITEN de closure
            if render_field(ui, "Title", &mut self.update_title, &mut self.edit_title) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "title") {
                        self.edit_title = val;
                    }
                }
            }

            if render_field(ui, "Artist", &mut self.update_artist, &mut self.edit_artist) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "artist") {
                        self.edit_artist = val;
                    }
                }
            }

            if render_field(ui, "Album", &mut self.update_album, &mut self.edit_album) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "album") {
                        self.edit_album = val;
                    }
                }
            }

            if render_field(ui, "Genre", &mut self.update_genre, &mut self.edit_genre) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "genre") {
                        self.edit_genre = val;
                    }
                }
            }

            if render_field(ui, "Jaar", &mut self.update_year, &mut self.edit_year) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "year") {
                        self.edit_year = val;
                    }
                }
            }

            if render_field(
                ui,
                "Componist",
                &mut self.update_composer,
                &mut self.edit_composer,
            ) {
                if let Some(path) = self.editing_track_path.clone() {
                    if let Some(val) = self.get_tag_value(&path, "composer") {
                        self.edit_composer = val;
                    }
                }
            }

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("💾 Opslaan in alle geselecteerde").strong())
                    .clicked()
                {
                    self.save_track_tags();
                }

                if let Some(status) = &self.save_status {
                    let color = if status.to_lowercase().contains("error")
                        || status.to_lowercase().contains("faalden")
                    {
                        Color32::RED
                    } else {
                        Color32::GREEN
                    };
                    ui.label(RichText::new(status).size(11.0).color(color));
                }
            });

            ui.separator();
            ui.add_space(5.0);

            ui.label(
                RichText::new("Klik op een bestand om de tags te inspecteren:")
                    .size(11.0)
                    .color(Color32::GRAY),
            );

            // Resizable split between "Bestanden" (left) and "Ruwe tags" (right)
            let avail = ui.available_rect_before_wrap();
            let sep_width = 4.0;
            let total_w = avail.width();
            let left_ratio = self.edit_panel_split;
            let left_w = ((total_w - sep_width) * left_ratio).max(80.0);
            let right_w = (total_w - sep_width - left_w).max(80.0);
            // Recalculate ratio after clamping so it stays accurate
            self.edit_panel_split = left_w / (left_w + right_w);

            // Left column — file list
            let left_rect =
                egui::Rect::from_min_size(avail.min, egui::vec2(left_w, avail.height()));
            let sep_rect = egui::Rect::from_min_size(
                egui::pos2(avail.min.x + left_w, avail.min.y),
                egui::vec2(sep_width, avail.height()),
            );
            let right_rect = egui::Rect::from_min_size(
                egui::pos2(avail.min.x + left_w + sep_width, avail.min.y),
                egui::vec2(right_w, avail.height()),
            );

            // ---- Left panel ----
            let mut left_ui = ui.child_ui(left_rect, *ui.layout(), None);
            left_ui.vertical(|ui| {
                ui.label(RichText::new("Bestanden:").strong().size(11.0));
                ScrollArea::vertical()
                    .id_source("batch_files_scroll")
                    .show(ui, |ui| {
                        // We clonen de lijst om borrow conflicts te voorkomen tijdens iteratie en mutatie
                        let tracks_to_edit_clone = self.tracks_to_edit.clone();
                        for track_path in &tracks_to_edit_clone {
                            ui.horizontal(|ui| {
                                let filename = Path::new(track_path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();

                                let is_selected =
                                    self.editing_track_path.as_deref() == Some(track_path);

                                if ui.selectable_label(is_selected, filename).clicked() {
                                    self.editing_track_path = Some(track_path.clone());
                                    self.refresh_raw_tags_for_path(track_path);
                                }

                                if ui
                                    .small_button("❌")
                                    .on_hover_text("Verwijder uit selectie")
                                    .clicked()
                                {
                                    self.selected_tracks.remove(track_path);
                                    self.tracks_to_edit.retain(|p| p != track_path);

                                    if self.editing_track_path.as_deref() == Some(track_path) {
                                        if let Some(first) = self.tracks_to_edit.first().cloned() {
                                            self.editing_track_path = Some(first.clone());
                                            self.refresh_raw_tags_for_path(&first);
                                        } else {
                                            self.editing_track_path = None;
                                        }
                                    }
                                }
                            });
                        }
                    });
            });

            // ---- Draggable separator ----
            let sep_response = ui.allocate_rect(sep_rect, egui::Sense::click_and_drag());
            if ui.is_rect_visible(sep_rect) {
                let stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;
                ui.painter()
                    .vline(sep_rect.center().x, sep_rect.y_range(), stroke);
            }
            // Visual feedback when hovered / dragged
            if sep_response.hovered() || sep_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if sep_response.dragged_by(egui::PointerButton::Primary) {
                let delta = sep_response.drag_delta().x;
                let new_left = left_w + delta;
                let new_ratio = new_left / (total_w - sep_width);
                self.edit_panel_split = new_ratio.clamp(0.15, 0.85);
            }

            // ---- Right panel ----
            let mut right_ui = ui.child_ui(right_rect, *ui.layout(), None);
            right_ui.vertical(|ui| {
                let current_file = self
                    .editing_track_path
                    .as_deref()
                    .unwrap_or("Geen bestand geselecteerd");
                ui.label(
                    RichText::new(format!(
                        "Ruwe tags: {}",
                        Path::new(current_file)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    ))
                    .strong()
                    .size(11.0),
                );
                ScrollArea::vertical()
                    .id_source("raw_tags_scroll")
                    .show(ui, |ui| {
                        if self.editing_track_path.is_some() {
                            ui.label(RichText::new(&self.raw_tags_display).monospace().size(10.0));
                        } else {
                            ui.label(
                                RichText::new("Klik op een bestand links.")
                                    .italics()
                                    .color(Color32::GRAY),
                            );
                        }
                    });
            });
        });
    }

    fn get_tag_value(&self, path: &str, key: &str) -> Option<String> {
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            for tag in tagged_file.tags() {
                match key {
                    "title" => {
                        if let Some(t) = tag.title() {
                            return Some(t.to_string());
                        }
                    }
                    "artist" => {
                        if let Some(a) = tag.artist() {
                            return Some(a.to_string());
                        }
                    }
                    "album" => {
                        if let Some(a) = tag.album() {
                            return Some(a.to_string());
                        }
                    }
                    "composer" => {
                        if let Some(item) = tag.get(&ItemKey::Composer) {
                            if let ItemValue::Text(t) = item.value() {
                                return Some(t.clone());
                            }
                        }
                    }
                    "genre" => {
                        let genres: Vec<String> = tag
                            .get(&ItemKey::Genre)
                            .into_iter()
                            .filter_map(|i| {
                                if let ItemValue::Text(t) = i.value() {
                                    Some(t.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !genres.is_empty() {
                            return Some(genres.join("; "));
                        }
                    }
                    "year" => {
                        if let Some(year_item) = tag.get(&ItemKey::Year) {
                            if let ItemValue::Text(t) = year_item.value() {
                                return Some(t.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn refresh_raw_tags_for_path(&mut self, path: &str) {
        self.raw_tags_display.clear();
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            let mut raw_text = String::new();
            for tag in tagged_file.tags() {
                raw_text.push_str(&format!("--- Tag Type: {:?} ---\n", tag.tag_type()));
                for item in tag.items() {
                    raw_text.push_str(&format!("{:?}: {:?}\n", item.key(), item.value()));
                }
            }
            self.raw_tags_display = if raw_text.is_empty() {
                "Geen tags gevonden.".to_string()
            } else {
                raw_text
            };
        }
    }
}
