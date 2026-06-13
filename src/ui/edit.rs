use std::path::Path;

use eframe::egui::{self, Color32, RichText, ScrollArea};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};

use super::app::MusicPlayerApp;

impl MusicPlayerApp {
    /// Tags wegschrijven naar bestand(en) met lofty.
    pub fn save_track_tags(&mut self) {
        if self.tracks_to_edit.is_empty() {
            return;
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for path in &self.tracks_to_edit {
            let result = (|| -> Result<(), String> {
                let mut tagged_file = Probe::open(path)
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

                // Verzamel data uit ALLE bestaande tags (niet alleen target type)
                // Zo missen we geen data die alleen in Id3v1 staat
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
                    // Verzamel data uit élke tag als fallback
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

                // Als de target_tag leeg is maar we hebben data uit andere tags, gebruik die dan
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

                // Verwijder ALLE oude tags en voeg alleen de nieuwe (target) tag toe
                // Dit voorkomt dat Id3v1 (of andere types) blijft staan met oude data.
                // Alleen de target_tag_type blijft bewaard (we schrijven die overschrijven we).
                // let target = target_tag_type;
                // for tag_type in [
                //     TagType::Id3v1,
                //     TagType::Id3v2,
                //     TagType::Mp4Ilst,
                //     TagType::VorbisComments,
                //     TagType::RiffInfo,
                // ] {
                //     if tag_type != target {
                //         tagged_file.remove(tag_type);
                //     }
                // }
                // // Verwijder ook target zodat we een schone tag hebben om te vullen
                // tagged_file.remove(target);

                // --- STANDAARD VELDEN ---
                if self.update_title {
                    tag.set_title(self.edit_title.clone());
                }
                if self.update_artist {
                    tag.set_artist(self.edit_artist.clone());
                }
                if self.update_album {
                    tag.set_album(self.edit_album.clone());
                }

                // --- MEERVOUDIGE GENRES (Gescheiden door ;) ---
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

                // --- JAAR ---
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

                // --- COMPONIST ---
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
                if target_tag_type != TagType::Id3v1 {
                    let _ = TagType::Id3v1.remove_from_path(path);
                }
                // DE MAGISCHE REGEL: vertel lofty om écht alle andere tags (zoals ID3v1)
                // uit het fysieke bestand te verwijderen bij het opslaan!
                let write_options = WriteOptions::new().remove_others(true);

                // Sla ALLEEN de 'tag' op, niet de 'tagged_file'.
                // Dit verwijdert automatisch ID3v1 en andere ongewenste tags van de schijf.
                tag.save_to_path(path, write_options)
                    .map_err(|e| format!("Save faalde: {:?}", e))?;

                Ok(())
            })();

            match result {
                Ok(_) => {
                    success_count += 1;
                    // Update in-memory library
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

        // Ververs raw_tags_display bij single track edit
        if success_count > 0 && self.tracks_to_edit.len() == 1 {
            if let Some(path) = &self.editing_track_path {
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

    /// Teken de Track Details / Batch Edit popup.
    pub fn show_track_details_popup(&mut self, ctx: &egui::Context) {
        let mut is_open = self.show_track_details;
        let popup_title = if self.tracks_to_edit.len() > 1 {
            format!(
                "Batch Edit: {} tracks geselecteerd",
                self.tracks_to_edit.len()
            )
        } else {
            "Track Details & Tags".to_string()
        };

        let mut path_to_remove: Option<String> = None;

        egui::Window::new(popup_title)
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if let Some(path) = &self.editing_track_path {
                    ui.label(RichText::new("File:").strong());
                    ui.label(RichText::new(path).size(12.0).color(Color32::GRAY));
                    ui.add_space(10.0);

                    if let Some(err) = &self.read_error {
                        ui.label(
                            RichText::new(format!("⚠️ Leesfout: {}", err))
                                .color(Color32::RED)
                                .strong(),
                        );
                        ui.add_space(5.0);
                    }

                    // Geselecteerde bestanden lijst (batch edit)
                    if self.tracks_to_edit.len() > 1 {
                        ui.label(
                            RichText::new("Geselecteerde bestanden:")
                                .strong()
                                .size(14.0),
                        );
                        ScrollArea::vertical()
                            .id_source("batch_files_scroll")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for track_path in &self.tracks_to_edit {
                                    ui.horizontal(|ui| {
                                        let filename = Path::new(track_path)
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy();

                                        ui.label(
                                            RichText::new(filename.to_string())
                                                .size(12.0)
                                                .color(Color32::GRAY),
                                        );
                                        ui.add_space(10.0);

                                        if ui.small_button("❌").clicked() {
                                            path_to_remove = Some(track_path.clone());
                                        }
                                    });
                                }
                            });
                        ui.separator();
                        ui.add_space(5.0);
                    }

                    // Editable velden
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_title, "");
                        ui.label("Title:");
                        ui.add_sized(
                            [400.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_title)
                                .interactive(self.update_title),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_artist, "");
                        ui.label("Artist:");
                        ui.add_sized(
                            [400.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_artist)
                                .interactive(self.update_artist),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_album, "");
                        ui.label("Album:");
                        ui.add_sized(
                            [400.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_album)
                                .interactive(self.update_album),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_genre, "");
                        ui.label("Genre (scheid met ';'):");
                        ui.add_sized(
                            [300.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_genre)
                                .interactive(self.update_genre),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_year, "");
                        ui.label("Jaar:");
                        ui.add_sized(
                            [400.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_year)
                                .interactive(self.update_year),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.update_composer, "");
                        ui.label("Componist:");
                        ui.add_sized(
                            [400.0, 20.0],
                            egui::TextEdit::singleline(&mut self.edit_composer)
                                .interactive(self.update_composer),
                        );
                    });

                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save to File").clicked() {
                            self.save_track_tags();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_track_details = false;
                        }
                        if let Some(status) = &self.save_status {
                            let color = if status.contains("Error") {
                                Color32::RED
                            } else {
                                Color32::GREEN
                            };
                            ui.label(RichText::new(status).color(color));
                        }
                    });

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(5.0);

                    ui.label(RichText::new("Alle Ruwe Tags (Read-Only):").strong());
                    ScrollArea::vertical()
                        .id_source("raw_tags_scroll")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(&self.raw_tags_display).monospace().size(12.0));
                        });
                }
            });

        self.show_track_details = is_open;

        // Verwijder track uit de lijst als op ❌ is geklikt
        if let Some(path) = path_to_remove {
            self.selected_tracks.remove(&path);
            self.tracks_to_edit.retain(|p| p != &path);

            if self.editing_track_path.as_deref() == Some(&path) {
                if let Some(first_path) = self.tracks_to_edit.first() {
                    self.editing_track_path = Some(first_path.clone());
                } else {
                    self.show_track_details = false;
                }
            }
        }
    }
}
