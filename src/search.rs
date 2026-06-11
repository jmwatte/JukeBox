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
                                    Some(
                                        track_genres
                                            .iter()
                                            .any(|tg| tg.to_lowercase() == genre.to_lowercase()),
                                    )
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
pub fn collect_years(library: &Library) -> Vec<(u32, usize)> {
    let mut map = std::collections::HashMap::new();
    for artist in &library.artists {
        for album in &artist.albums {
            for disk in &album.disks {
                for track in &disk.tracks {
                    if let Some(y) = track.year {
                        *map.entry(y).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut years: Vec<_> = map.into_iter().collect();
    years.sort_by(|a, b| a.0.cmp(&b.0));
    years
}

/// Filter the library to only include tracks from the given year
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
                    .filter(|t| t.year == Some(year))
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
