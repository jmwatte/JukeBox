use crate::models::{Album, Artist, Disk, Library, Track};

/// Helper function to split a genre string into individual genres
fn split_genres(genre_str: &str) -> Vec<String> {
    genre_str
        .split(&[';', '/', ',', '|', '\\'][..])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn filter_library(library: &Library, query: &str) -> Library {
    if query.trim().is_empty() {
        return library.clone();
    }

    let query_lower = query.to_lowercase();
    let mut filtered_artists = Vec::new();

    for artist in &library.artists {
        let mut filtered_albums = Vec::new();
        let artist_lower = artist.name.to_lowercase();

        for album in &artist.albums {
            let mut filtered_disks = Vec::new();
            let album_lower = album.title.to_lowercase();

            for disk in &album.disks {
                let mut filtered_tracks = Vec::new();

                for track in &disk.tracks {
                    let track_lower = track.title.to_lowercase();
                    let path_lower = track.path.to_lowercase();

                    if track_lower.contains(&query_lower)
                        || album_lower.contains(&query_lower)
                        || artist_lower.contains(&query_lower)
                        || path_lower.contains(&query_lower)
                    {
                        filtered_tracks.push(track.clone());
                    }
                }

                if !filtered_tracks.is_empty() {
                    filtered_disks.push(Disk {
                        name: disk.name.clone(),
                        tracks: filtered_tracks,
                    });
                }
            }

            if !filtered_disks.is_empty() {
                filtered_albums.push(Album {
                    title: album.title.clone(),
                    cover_path: album.cover_path.clone(),
                    disks: filtered_disks,
                    added_timestamp: album.added_timestamp,
                });
            }
        }

        if !filtered_albums.is_empty() {
            filtered_artists.push(Artist {
                name: artist.name.clone(),
                albums: filtered_albums,
            });
        }
    }

    Library {
        artists: filtered_artists,
    }
}

/// Collect all unique genres from the library, sorted alphabetically, with track counts
pub fn collect_genres(library: &Library) -> Vec<(String, usize)> {
    let mut map = std::collections::HashMap::new();
    let mut unknown_count = 0usize;

    for artist in &library.artists {
        for album in &artist.albums {
            for disk in &album.disks {
                for track in &disk.tracks {
                    if let Some(g) = &track.genre {
                        let genres = split_genres(g);
                        if genres.is_empty() {
                            unknown_count += 1;
                        } else {
                            for genre in genres {
                                *map.entry(genre).or_insert(0) += 1;
                            }
                        }
                    } else {
                        unknown_count += 1;
                    }
                }
            }
        }
    }

    if unknown_count > 0 {
        map.insert("Unknown".to_string(), unknown_count);
    }

    let mut genres: Vec<_> = map.into_iter().collect();
    genres.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    genres
}

/// Filter the library to only include tracks matching the given genre
pub fn filter_by_genre(library: &Library, genre: &str) -> Library {
    let mut filtered_artists = Vec::new();
    for artist in &library.artists {
        let mut filtered_albums = Vec::new();
        for album in &artist.albums {
            let mut filtered_disks = Vec::new();
            for disk in &album.disks {
                let filtered_tracks: Vec<Track> = disk
                    .tracks
                    .iter()
                    .filter(|track| {
                        if genre == "Unknown" {
                            track.genre.is_none()
                                || track
                                    .genre
                                    .as_deref()
                                    .map(|g| split_genres(g).is_empty())
                                    .unwrap_or(false)
                        } else {
                            track
                                .genre
                                .as_ref()
                                .and_then(|g| {
                                    let track_genres = split_genres(g);
                                    Some(track_genres.iter().any(|tg| tg == genre))
                                })
                                .unwrap_or(false)
                        }
                    })
                    .cloned()
                    .collect();
                if !filtered_tracks.is_empty() {
                    filtered_disks.push(Disk {
                        name: disk.name.clone(),
                        tracks: filtered_tracks,
                    });
                }
            }
            if !filtered_disks.is_empty() {
                filtered_albums.push(Album {
                    title: album.title.clone(),
                    cover_path: album.cover_path.clone(),
                    disks: filtered_disks,
                    added_timestamp: album.added_timestamp,
                });
            }
        }
        if !filtered_albums.is_empty() {
            filtered_artists.push(Artist {
                name: artist.name.clone(),
                albums: filtered_albums,
            });
        }
    }
    Library {
        artists: filtered_artists,
    }
}

/// Collect all unique years from the library, sorted ascending, with track counts
/// Tracks without a year tag are grouped under `None` ("Unknown").
pub fn collect_years(library: &Library) -> Vec<(Option<u32>, usize)> {
    let mut map = std::collections::HashMap::new();
    let mut unknown_count = 0usize;
    for artist in &library.artists {
        for album in &artist.albums {
            for disk in &album.disks {
                for track in &disk.tracks {
                    if let Some(y) = track.year {
                        *map.entry(y).or_insert(0) += 1;
                    } else {
                        unknown_count += 1;
                    }
                }
            }
        }
    }
    let mut years: Vec<_> = map.into_iter().map(|(y, c)| (Some(y), c)).collect();
    years.sort_by(|a, b| a.0.cmp(&b.0));
    if unknown_count > 0 {
        years.push((None, unknown_count));
    }
    years
}

/// Filter the library to only include tracks from the given year.
/// A year value of `0` is treated as "Unknown" (tracks without a year tag).
pub fn filter_by_year(library: &Library, year: u32) -> Library {
    let mut filtered_artists = Vec::new();
    for artist in &library.artists {
        let mut filtered_albums = Vec::new();
        for album in &artist.albums {
            let mut filtered_disks = Vec::new();
            for disk in &album.disks {
                let filtered_tracks: Vec<Track> = disk
                    .tracks
                    .iter()
                    .filter(|t| {
                        if year == 0 {
                            t.year.is_none()
                        } else {
                            t.year == Some(year)
                        }
                    })
                    .cloned()
                    .collect();
                if !filtered_tracks.is_empty() {
                    filtered_disks.push(Disk {
                        name: disk.name.clone(),
                        tracks: filtered_tracks,
                    });
                }
            }
            if !filtered_disks.is_empty() {
                filtered_albums.push(Album {
                    title: album.title.clone(),
                    cover_path: album.cover_path.clone(),
                    disks: filtered_disks,
                    added_timestamp: album.added_timestamp,
                });
            }
        }
        if !filtered_albums.is_empty() {
            filtered_artists.push(Artist {
                name: artist.name.clone(),
                albums: filtered_albums,
            });
        }
    }
    Library {
        artists: filtered_artists,
    }
}

/// Collect all unique composers from the library, sorted alphabetically, with track counts
pub fn collect_composers(library: &Library) -> Vec<(String, usize)> {
    let mut map = std::collections::HashMap::new();
    for artist in &library.artists {
        for album in &artist.albums {
            for disk in &album.disks {
                for track in &disk.tracks {
                    if let Some(ref c) = track.composer {
                        *map.entry(c.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut composers: Vec<_> = map.into_iter().collect();
    composers.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    composers
}

/// Filter the library to only include tracks by the given composer
pub fn filter_by_composer(library: &Library, composer: &str) -> Library {
    let mut filtered_artists = Vec::new();
    for artist in &library.artists {
        let mut filtered_albums = Vec::new();
        for album in &artist.albums {
            let mut filtered_disks = Vec::new();
            for disk in &album.disks {
                let filtered_tracks: Vec<Track> = disk
                    .tracks
                    .iter()
                    .filter(|t| {
                        t.composer
                            .as_deref()
                            .map(|c| c.to_lowercase() == composer.to_lowercase())
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                if !filtered_tracks.is_empty() {
                    filtered_disks.push(Disk {
                        name: disk.name.clone(),
                        tracks: filtered_tracks,
                    });
                }
            }
            if !filtered_disks.is_empty() {
                filtered_albums.push(Album {
                    title: album.title.clone(),
                    cover_path: album.cover_path.clone(),
                    disks: filtered_disks,
                    added_timestamp: album.added_timestamp,
                });
            }
        }
        if !filtered_albums.is_empty() {
            filtered_artists.push(Artist {
                name: artist.name.clone(),
                albums: filtered_albums,
            });
        }
    }
    Library {
        artists: filtered_artists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, genre: Option<&str>, year: Option<u32>) -> Track {
        Track {
            path: format!("dummy/{}/{}.flac", artist, title),
            title: title.to_string(),
            artist: Some(artist.to_string()),
            album_artist: None,
            track_number: 0,
            disc_number: 0,
            duration_secs: 180,
            genre: genre.map(|s| s.to_string()),
            year,
            composer: None,
        }
    }

    fn sample_library() -> Library {
        Library {
            artists: vec![
                Artist {
                    name: "Artist A".into(),
                    albums: vec![
                        Album {
                            title: "Album One".into(),
                            cover_path: None,
                            disks: vec![Disk {
                                name: "Default".into(),
                                tracks: vec![
                                    track("Song Alpha", "Artist A", Some("Rock"), Some(1999)),
                                    track("Song Beta", "Artist A", Some("Jazz"), Some(2001)),
                                ],
                            }],
                            added_timestamp: 0,
                        },
                        Album {
                            title: "Album Two".into(),
                            cover_path: None,
                            disks: vec![Disk {
                                name: "Default".into(),
                                tracks: vec![track("Song Gamma", "Artist A", None, None)],
                            }],
                            added_timestamp: 0,
                        },
                    ],
                },
                Artist {
                    name: "Artist B".into(),
                    albums: vec![Album {
                        title: "Solo Album".into(),
                        cover_path: None,
                        disks: vec![Disk {
                            name: "Default".into(),
                            tracks: vec![track(
                                "Only Song",
                                "Artist B",
                                Some("Classical"),
                                Some(2010),
                            )],
                        }],
                        added_timestamp: 0,
                    }],
                },
            ],
        }
    }

    #[test]
    fn filter_library_empty_query_returns_full() {
        let lib = sample_library();
        let result = filter_library(&lib, "");
        assert_eq!(result.artists.len(), 2);
    }

    #[test]
    fn filter_library_by_title() {
        let lib = sample_library();
        let result = filter_library(&lib, "Alpha");
        assert_eq!(result.artists.len(), 1);
        assert_eq!(
            result.artists[0].albums[0].disks[0].tracks[0].title,
            "Song Alpha"
        );
    }

    #[test]
    fn filter_library_by_artist() {
        let lib = sample_library();
        let result = filter_library(&lib, "Artist B");
        assert_eq!(result.artists.len(), 1);
        assert_eq!(result.artists[0].name, "Artist B");
    }

    #[test]
    fn filter_library_by_album() {
        let lib = sample_library();
        let result = filter_library(&lib, "Album Two");
        assert_eq!(result.artists.len(), 1);
        assert_eq!(result.artists[0].albums[0].title, "Album Two");
    }

    #[test]
    fn filter_library_case_insensitive() {
        let lib = sample_library();
        let result = filter_library(&lib, "alpha");
        assert_eq!(result.artists.len(), 1);
    }

    #[test]
    fn filter_library_no_match() {
        let lib = sample_library();
        let result = filter_library(&lib, "ZZZZ");
        assert!(result.artists.is_empty());
    }

    #[test]
    fn collect_genres_basic() {
        let lib = sample_library();
        let genres = collect_genres(&lib);
        assert!(genres.iter().any(|(g, _)| g == "Rock"));
        assert!(genres.iter().any(|(g, _)| g == "Jazz"));
        assert!(genres.iter().any(|(g, _)| g == "Classical"));
    }

    #[test]
    fn collect_genres_with_unknown() {
        let lib = sample_library();
        let genres = collect_genres(&lib);
        assert!(genres.iter().any(|(g, _)| g == "Unknown"));
    }

    #[test]
    fn filter_by_genre_rock() {
        let lib = sample_library();
        let result = filter_by_genre(&lib, "Rock");
        // Alleen Album One heeft een Rock-track (Song Alpha)
        assert_eq!(result.artists[0].albums.len(), 1);
        assert_eq!(result.artists[0].albums[0].title, "Album One");
        assert_eq!(
            result.artists[0].albums[0].disks[0].tracks[0].title,
            "Song Alpha"
        );
    }

    #[test]
    fn filter_by_genre_unknown() {
        let lib = sample_library();
        let result = filter_by_genre(&lib, "Unknown");
        assert_eq!(result.artists.len(), 1);
    }

    #[test]
    fn collect_years_basic() {
        let lib = sample_library();
        let years = collect_years(&lib);
        assert!(years.contains(&(Some(1999), 1)));
        assert!(years.contains(&(Some(2001), 1)));
        assert!(years.contains(&(Some(2010), 1)));
    }

    #[test]
    fn collect_years_has_unknown() {
        let lib = sample_library();
        let years = collect_years(&lib);
        assert!(years.contains(&(None, 1)));
    }

    #[test]
    fn test_filter_by_year() {
        let lib = sample_library();
        let result = filter_by_year(&lib, 1999);
        assert_eq!(result.artists[0].albums[0].disks[0].tracks.len(), 1);
    }

    #[test]
    fn test_filter_by_year_unknown() {
        let lib = sample_library();
        let result = filter_by_year(&lib, 0);
        assert_eq!(result.artists[0].albums[0].disks[0].tracks.len(), 1);
    }

    #[test]
    fn test_filter_by_composer() {
        let mut t = track("CT", "A", Some("Rock"), Some(2020));
        t.composer = Some("Bach".to_string());
        let lib = Library {
            artists: vec![Artist {
                name: "A".into(),
                albums: vec![Album {
                    title: "B".into(),
                    cover_path: None,
                    disks: vec![Disk {
                        name: "Default".into(),
                        tracks: vec![t],
                    }],
                    added_timestamp: 0,
                }],
            }],
        };
        let result = filter_by_composer(&lib, "bach");
        assert_eq!(result.artists[0].albums[0].disks[0].tracks.len(), 1);
    }

    #[test]
    fn test_collect_composers() {
        let mut t = track("CT", "A", Some("Rock"), Some(2020));
        t.composer = Some("Mozart".to_string());
        let lib = Library {
            artists: vec![Artist {
                name: "A".into(),
                albums: vec![Album {
                    title: "B".into(),
                    cover_path: None,
                    disks: vec![Disk {
                        name: "Default".into(),
                        tracks: vec![t],
                    }],
                    added_timestamp: 0,
                }],
            }],
        };
        let composers = collect_composers(&lib);
        assert_eq!(composers, vec![("Mozart".to_string(), 1)]);
    }

    #[test]
    fn split_genres_semicolon() {
        let result = split_genres("Rock;Jazz;Blues");
        assert_eq!(result, vec!["Rock", "Jazz", "Blues"]);
    }

    #[test]
    fn split_genres_slash() {
        let result = split_genres("Pop/Rock");
        assert_eq!(result, vec!["Pop", "Rock"]);
    }

    #[test]
    fn split_genres_empty() {
        let result = split_genres("");
        assert!(result.is_empty());
    }

    #[test]
    fn split_genres_trims() {
        let result = split_genres(" Rock ;  Jazz ");
        assert_eq!(result, vec!["Rock", "Jazz"]);
    }
}
