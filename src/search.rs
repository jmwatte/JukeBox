use crate::models::{Album, Artist, Disk, Library};

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

                    // Simple substring search - much more predictable!
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
