# Tileboard

A Rust terminal dashboard with a snapping grid, spanning tiles, and layouts you control.

Six built-in tiles cover CPU, memory and swap, network throughput, storage, local time, and system uptime. Tiles are ordinary Rust modules compiled into the application. Layouts, titles, colors, and tile settings live in TOML and can change without rebuilding.

![Tileboard at 120×30, rendered from the actual UI with sample data](docs/previews/wide.svg)

[Compact](docs/previews/compact.svg) · [Small](docs/previews/small.svg) · [Detailed](docs/previews/detail.svg) · [Amber](docs/previews/amber.svg) · [Monochrome](docs/previews/mono.svg) · [Editor](docs/previews/editor.svg) · [Settings](docs/previews/settings.svg)

Slate, amber, and monochrome themes use muted borders, padded cards, and slim usage bars. Larger tiles show oversized values, memory/network history graphs, and per-core CPU bars. Editing adds dotted grid guides and shaded placement previews; success notices fade after four seconds. Previews use sample data; live tiles display your machine's metrics.

## Run

Install [Rust](https://rustup.rs/) (1.95 or newer; current stable recommended), then:

```sh
cargo run --release --locked
```

Or install the binary:

```sh
cargo install --path . --locked
tileboard
```

Linux, macOS, and Windows are CI targets. Use a UTF-8 terminal; SSH works when the terminal forwards input and resize events. Mouse support depends on the terminal. Every editing operation is available from the keyboard.

Existing configuration files and layouts are preserved. Add new tiles with **e → a** (free some cells first), or try the six-tile layout with `cargo run --release -- --config examples/dashboard.toml`.

The first launch creates a default configuration in the platform's user configuration directory. To keep a configuration in a known location:

```sh
tileboard --config ./dashboard.toml
tileboard --config ./dashboard.toml --check
tileboard --print-default-config
```

`--check` validates an existing file without starting the UI or creating a file. Invalid configurations produce an error and remain untouched.

## Edit your dashboard

Press **e** to start an edit session. The shown profile stays pinned while editing, including when the terminal changes size. Tile changes affect that profile only; the theme applies to the whole dashboard. Press **p** to work on another profile. A dot next to EDIT indicates unsaved changes; undo returns to the profile and tile affected by the edit.

| Control | Action |
| --- | --- |
| Tab / Shift+Tab | Select next / previous tile |
| Arrow keys | Preview a move by one grid cell |
| Shift+arrow keys | Preview resizing by one grid cell |
| h / l | Shrink / grow width (alternative to Shift+arrows) |
| k / j | Shrink / grow height |
| Enter | Apply a valid preview |
| Mouse drag | Move a tile; release applies a valid move |
| Drag bottom-right ◢ | Resize a tile |
| a | Add a tile to a readable empty area |
| d / Delete | Remove the selected tile |
| t | Edit title, accent, tile-specific options, and refresh interval |
| c | Cycle slate, amber, and monochrome themes |
| u | Undo an applied edit (up to 100 changes) |
| p | Cycle responsive profiles |
| s | Save all profiles and leave edit mode |
| Esc | Discard a preview; otherwise cancel the entire edit session |
| ? | Show help |

Occupied cells block moves and resizes. The candidate turns red; the original placement stays unchanged. Correct the candidate or press Esc to discard it. Pending previews must be applied or discarded before saving. To add a tile to the initially full layout, shrink or remove an existing tile first.

In settings, **Tab** changes fields, **Ctrl+u** clears the current field, and **Enter** applies. Left/Right and Home/End move the text cursor; Backspace/Delete edit text, and bracketed paste is supported. In the accent field, Left/Right cycles colors. Long fields scroll to keep the cursor visible. Colors: `cyan`, `magenta`, `green`, `yellow`, `blue`, `red`, `white`. The clock accepts a Chrono/strftime format such as `%H:%M:%S` or `%I:%M %p`. Storage accepts an exact mount path; leave it empty to show all mounts. Network accepts an exact interface name; leave it empty to display the busiest interface (excluding `lo`/`lo0`). Unavailable interfaces stay unavailable instead of silently substituting another one.

Outside edit mode, **r** reloads TOML and **q** exits. **Ctrl+c** exits immediately and discards unsaved edits. Saving writes a temporary file in the config directory before replacing the old file. It serializes the configuration, so hand-written comments and formatting are not preserved. Avoid editing the TOML externally during an active UI edit session.

## Responsive rules

See [examples/dashboard.toml](examples/dashboard.toml) for a complete working configuration. Profiles are checked **in file order; the first match wins**. The last profile must be an unconditional fallback. The defaults use these rules:

| Profile | Matches | Visible tiles |
| --- | --- | --- |
| wide | ≥110 columns, ≥18 rows | All six, three across |
| compact | ≥70 columns, ≥22 rows | All six, two across |
| tall | ≥36 rows | All six stacked |
| small | ≥18 rows | CPU, clock, memory stacked |
| short | ≥70 columns | CPU, memory, clock side by side |
| minimal | Otherwise | CPU, using the available space |

All rules, placements, and visible tiles remain user-defined. The small profiles deliberately show fewer tiles rather than squeeze six unreadable cards onto the screen. Width and height refer to the whole terminal in character cells, including the dashboard header and footer.

```toml
version = 1
theme = "slate" # Also: "amber", "mono"

[[profiles]]
name = "wide"
min_width = 110
min_height = 24
# Optional inclusive limits:
# max_width = 250
# max_height = 80
# min_aspect = 2.0
# max_aspect = 5.0
columns = 6
rows = 4

[[profiles.tiles]]
id = "cpu-main"
kind = "cpu"
title = "CPU usage"
accent = "cyan"
refresh_ms = 1000
[profiles.tiles.placement]
column = 0
row = 0
column_span = 4
row_span = 2

[[profiles]]
name = "fallback"
columns = 2
rows = 4
# Add this profile's tiles here.
```

Positions start at zero. A tile may span any number of cells within its grid. Each profile defines its own tiles and settings; omit a tile from a profile to hide it at that size. Grid dimensions range from 1 to 64. Terminal cells are distributed proportionally, including leftover columns/rows. Aspect means terminal columns divided by rows, not a physical pixel ratio.

Grid dimensions and responsive rules are edited in TOML; tile placement and settings are also editable in the UI. Tiles below their minimum readable size show a compact placeholder. Terminals smaller than 26×10 show a resize prompt. There is no automatic rearranging, scrolling, or hidden collision resolution.

## Tile refresh intervals

Every tile instance has its own schedule. Press **e**, select a tile with **Tab**, press **t**, and edit **Refresh interval (ms)**. **Shift+Tab** from the title jumps to that field. Press **Enter** to apply, then **s** to save. You can also set `refresh_ms` in that tile's TOML table.

| Tile | Default interval |
| --- | --- |
| CPU, clock, network | 1 second |
| Memory | 2 seconds |
| System | 5 seconds |
| Storage | 10 seconds |

Intervals accept whole milliseconds from **250 to 86,400,000** (24 hours). Omitting `refresh_ms` uses the tile author's default. The bottom border shows the configured interval.

Each tile keeps its own last sample and history; redrawing or refreshing another tile does not change it. Only the visible profile requests updates. A tile samples when first shown, then waits its configured interval after each delivered update; polling and collection add some delay. Overdue tiles resume once without catch-up bursts. Changing its interval or data options restarts that tile's sampling state. Simultaneously due tiles can share system collection, while keeping independent displayed snapshots. A request taking over five seconds shows “delayed”; a deliberately long interval does not.

## Write a Rust tile

1. Add a module under `src/tiles/` implementing `Tile`.
2. Supply a `TileDefinition` with a unique kind, factory, configuration fields, validator, `default_refresh_ms`, and required metric `sources`.
3. Register it in `Registry::builtin()` in `src/tiles/mod.rs`.
4. Rebuild with Cargo. The tile appears in the **Add tile** menu.

The existing [clock tile](src/tiles/clock.rs) is a small example. The [CPU tile](src/tiles/cpu.rs) shows persistent state and sample updates.

```rust,ignore
impl Tile for MyTile {
    fn update(&mut self, config: &TileConfig, metrics: &Metrics) {
        // Update state on this tile instance's configured refresh schedule.
    }

    fn render(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        config: &TileConfig,
        metrics: &Metrics,
    ) {
        frame.render_widget(ratatui::widgets::Paragraph::new("Hello"), area);
    }
}
```

`sources` declares the needed `metrics::Source` values, such as `&[Source::Cpu]`; the worker returns only those sources. Use `&[]` for a self-contained tile such as the clock, which updates without worker I/O.

The host draws the title/border and supplies the tile's inner rectangle. Draw only within that rectangle. `minimum_size()` includes the surrounding border. Configurable tile options are strings, exposed through the settings form. Keep `update` and `render` quick; custom network or disk work belongs on a worker thread. Tiles are trusted application code and are not isolated from crashes. The optional `handle_key` hook is reserved for future interaction; the current dashboard does not dispatch input to tiles.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

System data comes from `sysinfo` on a worker thread. Sources are collected only when requested by a due tile. CPU sampling warms up once and shares a recent sample if requests are closer than the OS sampling minimum. Each network tile calculates throughput from byte counter deltas divided by its own actual elapsed sample time (two samples are needed initially), and resets its baseline when an interface appears or its counters reset. Auto selection reports one interface, avoiding the double-counting caused by adding physical, virtual, and VPN adapters. Memory usage is total minus available memory; cached/reclaimable memory is treated as available by the OS. Time follows the local clock. Storage lists mounted filesystems individually instead of summing them, since mounts can share backing devices. Missing mounts and delayed metrics are displayed explicitly.

Source structure:

- `config.rs`: TOML schema, profile rules, validation, atomic saves.
- `grid.rs`: spanning placement, collision checks, terminal/grid coordinate mapping.
- `app.rs`: independent tile schedules/caches, editor transactions, previews, undo, keyboard/mouse handling.
- `ui.rs`: dashboard and editor rendering.
- `metrics.rs`: background system sampling.
- `tiles/`: registry, tile API, and built-in implementations.
- `theme.rs`: theme presets, shared colors, byte and interval formatting.

For UI verification and review notes, see [docs/ux-review.md](docs/ux-review.md). Reproduce the real-terminal checks on Linux/macOS with Python 3.11+ and tmux:

```sh
cargo build --locked
python3 scripts/ui_smoke.py
cargo run --locked --example preview -- docs/previews
```

The smoke test uses isolated temporary configs. The preview example renders the actual UI with fixed sample data into SVG files; it never reads live system metrics.
