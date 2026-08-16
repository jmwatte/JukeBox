# Openstaande punten — JukeBoks

> Geconsolideerd uit de oude plannen (PLAN.md, PLAN_metadata_search.md, IMPROVEMENT_PLAN.md,
> MIGRATION_PLAN.md — verwijderd in aug 2026; geschiedenis staat in git).
> Dit is de enige nog geldige TODO-lijst voor de hoofd-app.

## Architectuur

- **H1 — Splits `MusicPlayerApp` op** in deel-staten: `PlaybackState`, `NavigationState`,
  `FilterState`, `EditState`, `WaveformState`. Elke module krijgt zijn eigen struct met
  alleen de relevante velden en methodes; `MusicPlayerApp` wordt coordinator.
  Bestanden: `src/ui/app.rs`, `src/ui/render.rs`, `src/ui/navigation.rs`, `src/ui/edit.rs`.
  Moette: groot (dagen). Impact: schaalbare architectuur.

- **M5 — Consolideer de dubbele A-B loop-implementatie.** Er zijn twee implementaties:
  1. Main player (`src/player.rs`) via rodio `seek()`
  2. Waveform player (`src/waveform_player.rs`) via rubato
  Synchronisatie via `waveform_pending_loop` is fragiel. Kies één bron van waarheid.
  Moette: 1 dag. Impact: minder bugs.

## Prestaties / UI

- **L1 — Optimaliseer filterfuncties** (`src/search.rs`): borrowing i.p.v. cloning;
  overweeg `Arc<Vec<Track>>` of een `TrackRef`-struct. Vooral nuttig bij grote libraries.

- **L3 — Verlaag polling-interval (30ms) in de audio-thread** of gebruik blocking
  `recv()` voor position-updates (`src/player.rs`). Vloeiendere UI.

## Metadata & search (nice-to-have)

- **Async search** — geen UI-freezes bij zeer grote libraries (nu: live-filtering,
  voldoet tot ~tienduizenden tracks).
- **Inverted index** tijdens de scan, geserialiseerd met bincode (alleen nuttig als
  async search daadwerkelijk nodig wordt).
