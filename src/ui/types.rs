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

#[derive(PartialEq, Clone)]
pub enum BrowseMode {
    Library,
    Genre,
    Recent,
    Selection,
}
