//! Favorieten: een simpele, persistente set van track-paden.
//!
//! Ontwerpkeuze (afgestemd met de gebruiker): favorieten zijn **alleen echte
//! bestandspaden van tracks**. Een favoriete artiest/album is daardoor impliciet
//! "alle tracks van dat niveau" — precies zoals de bestaande markeringen (M).
//! De "Favorieten"-view bouwt daar een selectie-bibliotheek van (`build_selection_library`).

const FAVORITES_FILE: &str = "favorites.json";

/// Laad favorieten van schijf.
pub fn load_favorites() -> Vec<String> {
    match std::fs::read_to_string(FAVORITES_FILE) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Sla favorieten weg naar schijf.
pub fn save_favorites(favorites: &[String]) {
    if let Ok(json) = serde_json::to_string_pretty(favorites) {
        let _ = std::fs::write(FAVORITES_FILE, json);
    }
}

/// Zet favoriet aan/uit voor alle gegeven paden (bv. alle tracks van het
/// huidige niveau) **zonder op te slaan**. Geeft `true` terug als de paden
/// zijn toegevoegd, `false` als ze zijn verwijderd (alle paden waren al favoriet).
pub fn toggle_favorites_in_memory(favorites: &mut Vec<String>, paths: &[String]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let all_favorite = paths.iter().all(|p| favorites.contains(p));
    if all_favorite {
        favorites.retain(|p| !paths.contains(p));
        false
    } else {
        for p in paths {
            if !favorites.contains(p) {
                favorites.push(p.clone());
            }
        }
        true
    }
}

/// Zet favoriet aan/uit voor alle gegeven paden en sla op.
/// Geeft `true` terug als de paden zijn toegevoegd, `false` als ze zijn verwijderd.
pub fn toggle_favorites(favorites: &mut Vec<String>, paths: &[String]) -> bool {
    let added = toggle_favorites_in_memory(favorites, paths);
    if !paths.is_empty() {
        save_favorites(favorites);
    }
    added
}

/// Pas de paden in de favorieten aan na een schijfletterwijziging
/// (zelfde patroon als `loops::remap_loops`). Slaat op als er iets wijzigde.
pub fn remap_favorites(old_root: &str, new_root: &str) -> Option<Vec<String>> {
    let mut favorites = load_favorites();
    let mut changed = false;
    for f in &mut favorites {
        let remapped = crate::scanner::remap_one_path(f, old_root, new_root);
        if remapped != *f {
            *f = remapped;
            changed = true;
        }
    }
    if changed {
        save_favorites(&favorites);
        log::info!("Favorieten bijgewerkt naar nieuwe schijfletter.");
        Some(favorites)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_adds_missing_paths() {
        let mut favs = vec!["a.flac".to_string()];
        assert!(toggle_favorites_in_memory(
            &mut favs,
            &["a.flac".into(), "b.flac".into()]
        ));
        // a stond er al, b wordt toegevoegd — geen duplicaten
        assert_eq!(favs, vec!["a.flac".to_string(), "b.flac".to_string()]);
    }

    #[test]
    fn toggle_removes_when_all_present() {
        let mut favs = vec!["a.flac".to_string(), "b.flac".to_string()];
        assert!(!toggle_favorites_in_memory(
            &mut favs,
            &["a.flac".into(), "b.flac".into()]
        ));
        assert!(favs.is_empty());
    }

    #[test]
    fn toggle_empty_returns_false() {
        let mut favs: Vec<String> = Vec::new();
        assert!(!toggle_favorites(&mut favs, &[]));
        assert!(favs.is_empty());
    }
}
