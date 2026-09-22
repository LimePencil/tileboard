# Tileboard

[![CI](https://github.com/LimePencil/tileboard/actions/workflows/ci.yml/badge.svg)](https://github.com/LimePencil/tileboard/actions/workflows/ci.yml) [![Release](https://github.com/LimePencil/tileboard/actions/workflows/release.yml/badge.svg)](https://github.com/LimePencil/tileboard/actions/workflows/release.yml)

A Rust terminal dashboard with a snapping grid, spanning tiles, and layouts you control.

Fourteen built-in tiles cover system metrics, processes, battery, temperatures, calendars, Git, services, weather, and usage quotas. Tiles are ordinary Rust modules compiled into the application. Layouts, titles, colors, and tile settings live in TOML and can change without rebuilding.

![Tileboard at 120×30, rendered from the actual UI with sample data](docs/previews/wide.svg)

[All tiles](docs/previews/gallery.svg) · [Swap preview](docs/previews/swap.svg) · [Compact](docs/previews/compact.svg) · [Small](docs/previews/small.svg) · [Detailed](docs/previews/detail.svg) · [Amber](docs/previews/amber.svg) · [Monochrome](docs/previews/mono.svg) · [Editor](docs/previews/editor.svg) · [Settings](docs/previews/settings.svg)

Slate, amber, and monochrome themes use muted borders, padded cards, and slim usage bars. Larger tiles show oversized values, memory/network history graphs, and per-core CPU bars. Editing adds dotted grid guides and shaded placement previews; success notices fade after four seconds. Previews use sample data; live tiles display your machine's metrics.

[Install](#install) · [Get started](#start-your-dashboard) · [Controls](#edit-your-dashboard) · [Tiles](#available-tiles) · [Layouts](#responsive-rules) · [Development](#development)

## Install

### Linux, macOS, and WSL

```sh
curl -fsSL https://raw.githubusercontent.com/LimePencil/tileboard/main/install.sh | sh
```

Installs to `~/.local/bin` and adds it to your shell's startup files. Open a new terminal, then run `tileboard`. The installer also prints a command to update your current terminal. Bash, Zsh, Fish, and POSIX-style shells are supported. WSL uses the Linux binary.

Requires `curl`, `tar`, and one of `sha256sum`, `shasum`, or `openssl`.

### Windows PowerShell

Run in **Windows PowerShell 5.1 or PowerShell 7+** on Windows x64:

```powershell
irm https://raw.githubusercontent.com/LimePencil/tileboard/main/install.ps1 | iex
tileboard
```

Installs to `$env:LOCALAPPDATA\Tileboard\bin` and updates both Windows User PATH and the current PowerShell session. Restart other terminal applications to pick up the change. No administrator privileges, Git Bash, or execution-policy changes are required for this command.

If you prefer Git Bash, MSYS2, or Cygwin, use the `install.sh` command above. That installer requires `unzip`, installs to `~/.local/bin`, and updates Windows User PATH as well.

### Versions and options

Both installers detect the platform, download the latest release, and verify its SHA-256 checksum before replacing an existing binary. Rerun your installation command to upgrade; your layouts and settings are preserved. You can inspect [install.sh](install.sh) or [install.ps1](install.ps1) before running them.

To pin a release or choose an installation directory:

```sh
curl -fsSL https://raw.githubusercontent.com/LimePencil/tileboard/main/install.sh | sh -s -- \
  --version 0.1.0 --bin-dir "$HOME/.local/bin"
```

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/LimePencil/tileboard/main/install.ps1))) `
  -Version 0.1.0 -BinDir "$env:LOCALAPPDATA\Tileboard\bin"
```

| Option | Shell installer | PowerShell installer |
| --- | --- | --- |
| Release version | `--version VERSION` | `-Version VERSION` |
| Absolute install directory | `--bin-dir PATH` | `-BinDir PATH` |
| Leave PATH unchanged | `--no-modify-path` | `-NoModifyPath` |
| Override detected shell | `--shell bash`, `zsh`, `fish`, or `sh` | Not needed |
| Choose startup file | `--profile /absolute/path` | Not needed |

The shell installer respects `ZDOTDIR` for Zsh and `XDG_CONFIG_HOME` for Fish. To uninstall, delete the installed binary and remove its PATH entry. For shell installs, remove the corresponding `# Tileboard` lines from your startup files. Configuration files are kept separately.

### Manual download and supported platforms

Download an archive from [GitHub Releases](https://github.com/LimePencil/tileboard/releases/latest), extract it, then run `./tileboard` on Linux/macOS or `.\tileboard.exe` in PowerShell. Archives include documentation and example layouts; `SHA256SUMS` lists their checksums. Manual extraction does not update PATH.

| Platform | Architecture | Requirement / archive target |
| --- | --- | --- |
| Linux | x64, ARM64 | glibc 2.35+; `x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` |
| macOS | Intel, Apple Silicon | macOS 11+; `x86_64-apple-darwin` / `aarch64-apple-darwin` |
| Windows | x64 | `x86_64-pc-windows-msvc` |

Release binaries are unsigned. Alpine/musl, Windows ARM64, and other unsupported targets need a source build.

### Build from source

Install [Rust](https://rustup.rs/) 1.95 or newer, then clone and install:

```sh
git clone https://github.com/LimePencil/tileboard.git
cd tileboard
cargo install --path . --locked
tileboard
```

For development, use `cargo run --release --locked` from the checkout.

## Start your dashboard

Run `tileboard` in a UTF-8 terminal. The first launch creates a default configuration in your platform's user configuration directory. SSH works when the terminal forwards input and resize events; all editing operations are available from the keyboard.

Press **e** to edit, **Tab** to select a tile, and the **arrow keys** to move or swap it. Press **t** for settings, **a** to add, **r** to replace, or **c** to change theme. Press **s** to save. Outside edit mode, **r** reloads your configuration and **q** exits.

To use a configuration at a specific location:

```sh
tileboard --config ./dashboard.toml
tileboard --config ./dashboard.toml --check
tileboard --print-default-config
```

The first command creates a default layout if the file does not exist. `--check` only validates an existing file; it never creates or changes one. `--print-default-config` prints the default TOML.

Example layouts are included in the [release archives](https://github.com/LimePencil/tileboard/releases/latest) and [source checkout](examples). With `tileboard` on PATH, run these commands from the directory containing `examples`:

```sh
tileboard --config examples/dashboard.toml
tileboard --config examples/all-tiles.toml
```

The full gallery works best at **160×54** or larger. The one-line installers install only the executable; download an archive or clone the repository to get these examples. Relative file and repository paths resolve from the directory where you run Tileboard.

## Edit your dashboard

Press **e** to start an edit session. The shown profile stays pinned while editing, including when the terminal changes size. Tile changes affect that profile only; the theme applies to the whole dashboard. Press **p** to work on another profile. A dot next to EDIT indicates unsaved changes; undo returns to the profile and tile affected by the edit.

| Control | Action |
| --- | --- |
| Tab / Shift+Tab | Select next / previous tile |
| Arrow keys | Move through free cells; swap when entering an occupied tile |
| Shift+arrow keys | Preview resizing by one grid cell |
| h / l | Shrink / grow width (alternative to Shift+arrows) |
| k / j | Shrink / grow height |
| Enter | Apply a valid preview |
| Mouse drag | Move into free space or onto another tile to swap; release applies |
| Drag bottom-right ◢ | Resize a tile |
| a | Add a tile to a readable empty area |
| r | Replace the selected tile, keeping its grid slot |
| d / Delete | Remove the selected tile |
| t | Edit title, accent, tile-specific options, and refresh interval |
| c | Cycle slate, amber, and monochrome themes |
| u | Undo an applied edit (up to 100 changes) |
| p | Cycle responsive profiles |
| s | Save all profiles and leave edit mode |
| Esc | Discard a preview; otherwise cancel the entire edit session |
| ? | Show help |

**A full grid is rearrangeable.** Move toward an occupied tile with arrows, or drag onto it, to preview a swap. Both tiles appear in their proposed slots with a “swap preview” label. **Enter** applies a keyboard preview; releasing the mouse applies a drag. **Esc** discards the preview and **u** undoes the entire swap. Tiles exchange their complete grid rectangles, including spans when the sizes differ; their settings and refresh intervals stay with them.

Moving into free space keeps the grid placement workflow. Resizing still requires free cells; a blocked resize or out-of-bounds move turns red without changing the saved layout. Pending previews must be applied or discarded before saving. Use **r** while editing to replace a tile without freeing cells: the new kind uses its default options and refresh interval, keeps the old slot and accent, and participates in undo/save/cancel.

In settings, **Tab** changes fields, **Ctrl+u** clears the current field, and **Enter** applies. Left/Right and Home/End move the text cursor; Backspace/Delete edit text, and bracketed paste is supported. In the accent field, Left/Right cycles colors. Long fields scroll to keep the cursor visible. Colors: `cyan`, `magenta`, `green`, `yellow`, `blue`, `red`, `white`. The clock accepts a Chrono/strftime format such as `%H:%M:%S` or `%I:%M %p`. Storage accepts an exact mount path; leave it empty to show all mounts. Network accepts an exact interface name; leave it empty to display the busiest interface (excluding `lo`/`lo0`). Unavailable interfaces stay unavailable instead of silently substituting another one.

Outside edit mode, **r** reloads TOML and **q** exits. **Ctrl+c** exits immediately and discards unsaved edits. Saving writes a temporary file in the config directory before replacing the old file. It serializes the configuration, so hand-written comments and formatting are not preserved. Avoid editing the TOML externally during an active UI edit session.

## Available tiles

| Kind | Shows / settings | Default refresh |
| --- | --- | --- |
| `cpu` | Total CPU, history, per-core bars | 1 s |
| `memory` | RAM, swap, history | 2 s |
| `network` | Receive/send rates; optional interface | 1 s |
| `storage` | Free/used space; optional mount | 10 s |
| `clock` | Local time; strftime format | 1 s |
| `system` | Uptime, host, OS, CPU count | 5 s |
| `processes` | Top processes; sort by `cpu` or `memory`, name filter | 2 s |
| `battery` | Charge, state, health, remaining-time estimate when available | 30 s |
| `temperature` | Hottest sensors first; name filter, Celsius/Fahrenheit | 5 s |
| `calendar` | Current month with today highlighted; Monday/Sunday week start | 1 min |
| `git` | Branch/upstream summary, changed/staged/untracked files; repository path | 5 s |
| `service` | HTTP status and response time; URL checked with HEAD | 30 s |
| `weather` | Temperature, feels-like, humidity, conditions; latitude/longitude and units | 10 min |
| `usage` | Used/limit, percentage, remaining quota, reset text; JSON file or endpoint | 1 min |

The gallery contains example sources: this repository, a local health endpoint, Seoul weather, and a clearly labeled sample usage report. Existing user layouts are never replaced automatically. The gallery's calendar and process tiles need wider/taller slots; smaller slots show “Enlarge tile.”

Process CPU uses **100% per logical CPU**, so a multithreaded process can exceed 100%; `CPU¹` marks this convention. Its first sample warms up. Temperature and battery availability depend on OS/hardware support; missing hardware is shown explicitly. Battery collection uses [starship-battery](https://docs.rs/starship-battery/latest/starship_battery/).

Git requires the `git` executable and reads the local checkout without fetching. Relative file/repository paths are resolved from the application's working directory. Service checks use HEAD: only 2xx is healthy, and redirects are reported without being followed. Weather uses [Open-Meteo](https://open-meteo.com/en/docs); blank coordinates make no request. Source workers are separate from system sampling and from one another, with five-second HTTP/Git timeouts. Tiles of the same external source share that source's worker.

### Usage reports

The usage tile is a configurable report reader; it does **not** automatically connect to an account or estimate a provider's quota. Set `source` to a regular JSON file or HTTP(S) endpoint returning:

```json
{"used": 1250, "limit": 5000, "unit": "requests", "reset_at": "2026-10-01T00:00:00Z"}
```

`used` must be nonnegative and `limit` positive. `unit` and `reset_at` are optional; reset text is displayed as provided. For an authenticated endpoint, set `token_env` to the name of an environment variable containing a bearer token. The token itself stays outside TOML. An account-specific exporter or endpoint must supply the report; [examples/usage.json](examples/usage.json) is sample data. Missing, malformed, oversized, or unreachable reports show an error instead of a fabricated usage value.

## Tile refresh intervals

Every tile instance has its own schedule. Press **e**, select a tile with **Tab**, press **t**, and edit **Refresh interval (ms)**. **Shift+Tab** from the title jumps to that field. Press **Enter** to apply, then **s** to save. You can also set `refresh_ms` in that tile's TOML table.

Default intervals are listed in [Available tiles](#available-tiles).

Intervals accept whole milliseconds from **250 to 86,400,000** (24 hours). Omitting `refresh_ms` uses the tile author's default. The bottom border shows the configured interval.

Each tile keeps its own last sample and history; redrawing or refreshing another tile does not change it. Only the visible profile requests updates. A tile samples when first shown, then waits its configured interval after each delivered update; polling and collection add some delay. Overdue tiles resume once without catch-up bursts. Changing its interval or data options restarts that tile's sampling state. Simultaneously due tiles can share system collection, while keeping independent displayed snapshots. A request taking over five seconds shows “delayed”; a deliberately long interval does not.

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

Grid dimensions and responsive rules are edited in TOML; tile placement and settings are also editable in the UI. Tiles below their minimum readable size show a compact placeholder. Terminals smaller than 26×10 show a resize prompt. Swaps affect only the selected tile and its destination. There is no cascading rearrangement or scrolling.

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

For OS sources, `sources` declares the needed `metrics::Source` values, such as `&[Source::Cpu]`; the worker returns only those sources. External and battery sources use dedicated workers and must be the sole source in their definition; `SampleRequest.options` carries each tile's settings. Use `&[]` for a self-contained tile such as the clock, which updates without worker I/O.

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
- `metrics.rs`: background system sampling and source workers.
- `integrations.rs`: Git, HTTP checks, weather, and usage-report collection.
- `tiles/`: registry, tile API, and built-in implementations.
- `theme.rs`: theme presets, shared colors, byte and interval formatting.

For UI verification and review notes, see [docs/ux-review.md](docs/ux-review.md). Reproduce the real-terminal checks on Linux/macOS with Python 3.11+ and tmux:

```sh
cargo build --locked
python3 scripts/ui_smoke.py
cargo run --locked --example preview -- docs/previews
```

The smoke test uses isolated temporary configs. The preview example renders the actual UI with fixed sample data into SVG files; it never reads live system metrics.

## CI and releases

Two GitHub Actions workflows serve different purposes:

- **CI** runs for pull requests and pushes/merges to `main`: formatting, Clippy, tests on Linux/macOS/Windows, shell and PowerShell installer checks, config checks, and a Linux terminal smoke test. Tags do not trigger this workflow.
- **Release** runs when a GitHub release is published. It verifies the tag matches `Cargo.toml` and `Cargo.lock`, runs native tests, builds five platform binaries, checks their version and example configs, and uploads archives plus `SHA256SUMS` once all builds succeed. Publishing a draft triggers the workflow; merely saving a draft does not.

To release a new version:

1. Update `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md`, merge the changes, and wait for CI to pass.
2. Create and push an annotated `vX.Y.Z` tag at that commit.
3. Publish a GitHub release for the tag. Release assets appear after the **Release** workflow succeeds.

The Release workflow also has a manual **Run workflow** input for rebuilding an existing release tag after an infrastructure failure. Normal publishing uses `GITHUB_TOKEN`; no personal access token is required. Cargo registry publication is disabled; versions are distributed through GitHub Releases.
