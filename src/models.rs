use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: u32,
    pub disc_number: u32,
    pub duration_secs: u32,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub composer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Disk {
    pub name: String, // bijv. "cd1", "disc2" of leeg als er geen submappen zijn
    pub tracks: Vec<Track>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Album {
    pub title: String,
    pub cover_path: Option<String>,
    pub disks: Vec<Disk>, // Niveau 3: CD/Disk-niveau of Albums-in-box
    #[serde(default)] // Fallback just in case
    pub added_timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Artist {
    pub name: String,
    pub albums: Vec<Album>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Library {
    pub artists: Vec<Artist>,
}

//#[derive(Debug, Clone)]
// pub struct SearchResult {
