# UI and UX review

Reviewed the running application in a real tmux terminal and rendered the same UI with sample data for visual inspection. This is an engineering usability pass, not a study with external users.

## Findings and changes

| Observed problem | Change | Verification |
| --- | --- | --- |
| Long settings fields hid the text being entered | Cursor navigation, grapheme-aware deletion, horizontal scrolling, bracketed paste | Unicode input regression tests and real-terminal paste/save |
| Short terminals hid save/cancel behind a long control list | Contextual footer with essential actions first | 26×10 and 38×16 render/terminal checks |
| Undo after switching profiles changed an unseen layout | Undo restores the affected profile and selection | Cross-profile regression test |
| New tiles could occupy a cell too small to read | Placement checks the rendered minimum dimensions; prefers a useful span | Add-tile regression test and terminal picker flow |
| Three large cards left little useful information on screen | Six built-in tiles and height-aware defaults | Live CPU, memory, network, disk, clock, and system data |
| Bright borders and solid bars competed with the content | Shared slate palette, muted borders, soft accents, gutters, padding, slim gauges | Visual inspection of wide, compact, small, and settings previews |
| Empty storage data looked permanently “loading” | Separate loading, unavailable mount, and no-mount states | Render paths distinguish initial and collected snapshots |

## Packed-grid movement and new sources

Moving onto an occupied tile previews an exchange of complete slots, including spans. Both affected tiles render in their future positions, with preview labels. Keyboard Enter and mouse release commit one undoable change. Resizing remains constrained by free space. The `r` editor action replaces a tile in its slot, so the full default grid can use any new tile immediately.

The registry now includes processes, battery, temperatures, calendar, Git status, HTTP service checks, weather, and a configurable usage-report reader. The [gallery](previews/gallery.svg) renders all fourteen with fixtures; [swap preview](previews/swap.svg) shows a packed-grid exchange. Report parsing and service checks use local fixtures in tests; no credentials or external network are needed to run tests. Hardware-specific values are not fabricated when a sensor or battery is absent.

## Visual and refresh improvements

Larger cards display oversized values, memory/network histories, and per-core CPU bars. Shared border annotations show each tile's interval. The editor uses a double border for selection, dotted empty cells, and shaded valid/blocked previews with span dimensions. Save/reload notices fade while errors remain visible. Slate, amber, and monochrome themes participate in the existing save/undo/cancel flow.

Each visible tile independently schedules collection and retains its last snapshot. The refresh field accepts 250 ms through 24 hours; defaults vary by metric. Pending requests are deduplicated, and generation checks reject responses from replaced configurations. Network rate calculations use each instance's own sampling window. UI redraws never advance a tile's sample.

## New useful tiles

- **Memory:** usage percentage, used/total RAM, available memory, and swap usage (including swap disabled).
- **Network:** receive/send rates and interface name. Auto mode selects the busiest interface except `lo`/`lo0`; an exact interface can be configured. Rates use elapsed sample time, and new/reset counters warm up instead of producing a spike.
- **System:** uptime, hostname, OS description, and logical CPU count.

These are compiled Rust modules using the existing registry/API. They do not require network credentials or external services. Collection remains off the UI thread. Existing TOML files load without layout migration.

## Repeatable checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
python3 scripts/ui_smoke.py
cargo run --locked --example preview -- docs/previews
cargo build --release --locked
```

The original 44 Rust tests cover geometry, config preservation, responsive selection, editor transactions, mouse resizing/collisions, Unicode fields, new tile values/unavailable states, network counter resets, legacy configurations, independent instance deadlines, paused profiles, stale response rejection, per-instance network rates, interval validation/persistence, themes, transient notices, packed-grid swaps, replacement, new tile states, JSON validation, local Git collection, and worker isolation during a stalled HTTP check.

The tmux smoke script creates an isolated terminal and temporary config, then exercises six live tiles, five terminal shapes, full-grid swap previews, blocked resizing, replacement/undo, mouse dragging, settings/paste, refresh interval and theme persistence, save/cancel, adding a network tile, a missing interface, recovery from a bad config reload, independent 250 ms and 4 s clocks, and clean exit. A separate 160×54 live-gallery run checked the new process collector, hardware availability states, local Git, usage-file loading, and setup prompts.

Visual review sizes: **160×54** (all fourteen tiles), **120×44**, **120×30** (slate, amber, monochrome, and edit preview), **80×24**, **42×28**, and **38×16**. Additional layout/readability checks: **120×12**, **38×40**, and **26×10**, plus tiny windows that display the resize prompt. Full keyboard/mouse runtime checks were performed on Linux. This update passed `cargo check --all-targets` for macOS ARM64 and Windows x86-64, using temporary Zig 0.14.1 C compiler wrappers for the TLS dependency. These checks do not link or execute native binaries; native runtime remains unverified.

The SVGs in [previews](previews/) are generated from Ratatui's actual rendered cell buffer with fixed sample metrics. They contain no live machine details. Font rendering may differ slightly from your terminal.

## Saved profiles and grid settings

The profile picker is available with **p** outside edit mode, with **[** / **]** for immediate previous/next selection. Manual selection is saved and survives terminal resizing and restarts; **Auto** restores responsive matching. The subtitle shows the mode and profile even on small screens. Failed selection saves preserve the current selection, and switching refuses to overwrite externally modified configuration.

In edit mode, **g** opens profile/grid/rule settings and **n** creates a named independent copy. Copies start as manual-only layouts, leaving Auto matching unchanged. Both operations join the existing save/undo/cancel transaction. Grid shrinkage, duplicate names, invalid bounds, and loss of the required automatic fallback are rejected before changing the live configuration. Profile forms reuse the Unicode-aware settings editor and scroll the active field into view.

The [profile picker](previews/profiles.svg) and [profile settings](previews/profile-settings.svg) previews use the actual UI with fixed fixture metrics. Regression tests cover saved selections, copies, reloads, undo/cancel, validation failures, small-screen forms, and stalled same-kind external requests.

## Remaining boundaries

- CI runs native tests on Linux, macOS, and Windows; the Release workflow also tests each binary target before packaging. Terminal-specific mouse behavior still needs verification in the terminal applications being used.
- Small default profiles show a subset of tiles, as declared in TOML. Editing a different profile while the terminal is small can still yield compact placeholders; enlarge the terminal to preview that layout at its intended size.
- Tile content remains display-only. Profile order is edited in TOML; names, grid dimensions, rules, tile placement, and tile settings are editable in the UI.
- Adding a separate tile needs free space. Replacement works in occupied slots, and swaps work on a full grid without changing unrelated tiles.
