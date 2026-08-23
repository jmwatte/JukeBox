# TODO — Favorieten & Recente albums

Twee functies die samen in deze ronde worden opgeleverd. Keuzes die eerder
met de gebruiker zijn afgestemd: favorieten als **set van track-paden**
(optie 1, met niveaus artiest/album/disk/track) en de recente lijst als
**platte "Nieuwste albums"-lijst**.

## 1. Favorieten (⭐)

- [x] Nieuwe module `src/favorites.rs`: laden/opslaan (`favorites.json`),
      toggle op paden, remap bij schijfletterwijziging (patroon van loops)
- [x] Shortcuts: `ToggleFavorite` (`F`) en `FavoritesBrowse` (`Shift+F`)
- [x] Toggle-favoriet op het huidige niveau (artiest/album/disk/track)
- [x] "Favorieten"-view via `build_selection_library` + terug met `Esc`
- [x] ⭐-indicatie in de tracklijst
- [x] Schijfletter-remap voor `favorites.json` (net als `loops.json`)
- [x] Help-scherm + `MANUAL.md`

## 2. Recente albums (🕒 platte "Nieuwste albums"-lijst)

- [x] `enter_recent_mode` bewaart ook de artiestnaam
- [x] Render de platte lijst (hoes + artiest – album + toevoegdatum)
- [x] Lege lijst → statusbericht "Geen albums gevonden" + terug naar normale view
- [x] Help-scherm + `MANUAL.md`
