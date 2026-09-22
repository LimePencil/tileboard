# Changelog

## 0.1.0 — 2026-09-22

First release of Tileboard, a customizable Rust terminal dashboard.

- Responsive layouts defined in TOML, with snapping grids and tiles spanning multiple cells.
- Keyboard and mouse editing, full-grid swaps, replacement in place, previews, undo, and save/cancel.
- Fourteen built-in tiles: CPU, memory/swap, storage, network, clock, system, processes, battery, temperatures, calendar, Git, service health, weather, and usage/quota reports.
- Independent refresh intervals per tile, from 250 milliseconds to 24 hours.
- Slate, amber, and monochrome themes; adaptive values, history graphs, and per-core CPU bars.
- Config validation, legacy-layout preservation, example dashboards, and a tile-authoring API for compiled Rust modules.
- Separate validation and release workflows. Release builds provide Linux, macOS, and Windows archives with SHA-256 checksums.

Weather needs coordinates; Git and service tiles need their source settings. Usage reads a configured JSON file or endpoint; it does not automatically connect to an account. Battery and sensor readings depend on available hardware. Tile content is display-only.
