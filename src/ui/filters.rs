use crate::models::{Album, Library};
use crate::ui::types::FilterNode;

pub struct FilterState {
    pub filter_path: Vec<FilterNode>,
    pub filter_step: usize,
    pub genres: Vec<(String, usize)>,
    pub selected_genre: usize,
    pub selected_genre_name: Option<String>,
    pub sort_by_date: bool,
    pub recent_albums: Vec<(u64, Album)>,
    pub selected_recent: usize,
    pub years: Vec<(Option<u32>, usize)>,
    pub selected_year: usize,
    pub composers: Vec<(String, usize)>,
    pub selected_composer: usize,
    pub cached_filtered: Option<Library>,
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            filter_path: Vec::new(),
            filter_step: 0,
            genres: Vec::new(),
            selected_genre: 0,
            selected_genre_name: None,
            sort_by_date: false,
            recent_albums: Vec::new(),
            selected_recent: 0,
            years: Vec::new(),
            selected_year: 0,
            composers: Vec::new(),
            selected_composer: 0,
            cached_filtered: None,
        }
    }
}
