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

/// Een filter-node in de filter-pipeline.
/// - `None` = de node is een Picker (de gebruiker moet nog een waarde kiezen)
/// - `Some(...)` = de gebruiker heeft een waarde geselecteerd (actief filter)
#[derive(PartialEq, Clone, Debug)]
pub enum FilterNode {
    Genre(Option<String>),
    Year(Option<u32>),
    Composer(Option<String>),
}

impl FilterNode {
    /// Geeft een leesbare naam voor de breadcrumb
    pub fn display_name(&self) -> String {
        match self {
            FilterNode::Genre(Some(name)) => format!("Genre: {}", name),
            FilterNode::Genre(None) => "Genres".into(),
            FilterNode::Year(Some(y)) => format!("{}", y),
            FilterNode::Year(None) => "Jaartallen".into(),
            FilterNode::Composer(Some(name)) => name.clone(),
            FilterNode::Composer(None) => "Componisten".into(),
        }
    }

    /// Picker-header voor de weergave
    pub fn picker_name(&self) -> &str {
        match self {
            FilterNode::Genre(_) => "Genres",
            FilterNode::Year(_) => "Jaartallen",
            FilterNode::Composer(_) => "Componisten",
        }
    }

    /// Wis de gemaakte selectie (zet terug naar None = Picker mode)
    pub fn clear(&mut self) {
        match self {
            FilterNode::Genre(v) => *v = None,
            FilterNode::Year(v) => *v = None,
            FilterNode::Composer(v) => *v = None,
        }
    }
}
