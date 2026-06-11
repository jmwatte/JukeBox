#[derive(PartialEq, Clone, Debug)]
pub enum NavLevel {
    Artist,
    Album,
    Disk,
    Track,
}

#[derive(PartialEq, Clone, Debug)]
pub enum ViewMode {
    Tracklist,
    AlbumCover,
}

/// Een laag op de filter-stapel.
/// - Picker-varianten tonen een keuzelijst (bv. alle genres) boven op de huidige set.
/// - Filter-varianten perken de set in.
#[derive(PartialEq, Clone, Debug)]
pub enum Layer {
    /// Geen filters, toon de volledige bibliotheek
    Root,
    /// Toon een picker-lijst met alle genres (uit de huidige set)
    GenrePicker,
    /// Toon een picker-lijst met alle jaren (uit de huidige set)
    YearPicker,
    /// Toon een picker-lijst met alle componisten (uit de huidige set)
    ComposerPicker,
    /// Toon de nieuwste albums (uit de huidige set)
    RecentAlbums,
    /// Actief filter: alleen tracks met dit genre
    Genre(String),
    /// Actief filter: alleen tracks uit dit jaar
    Year(u32),
    /// Actief filter: alleen tracks van deze componist
    Composer(String),
    /// Actief filter: alleen gemarkeerde tracks
    Selection,
}

impl Layer {
    /// Geeft een leesbare naam voor de breadcrumb
    pub fn display_name(&self) -> String {
        match self {
            Layer::Root => "Bibliotheek".into(),
            Layer::GenrePicker => "Genres".into(),
            Layer::YearPicker => "Jaartallen".into(),
            Layer::ComposerPicker => "Componisten".into(),
            Layer::RecentAlbums => "Nieuwste".into(),
            Layer::Genre(name) => format!("Genre: {}", name),
            Layer::Year(y) => format!("{}", y),
            Layer::Composer(name) => name.clone(),
            Layer::Selection => "Selectie".into(),
        }
    }

    /// Is dit een picker? (toont keuzelijst ipv library-navigatie)
    pub fn is_picker(&self) -> bool {
        matches!(
            self,
            Layer::GenrePicker | Layer::YearPicker | Layer::ComposerPicker | Layer::RecentAlbums
        )
    }
}
