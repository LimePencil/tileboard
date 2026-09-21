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

The 22 Rust tests cover geometry, config preservation, responsive selection, editor transactions, mouse resizing/collisions, Unicode fields, new tile values/unavailable states, network counter resets, and legacy configurations.

The tmux smoke script creates an isolated terminal and temporary config, then exercises six live tiles, five terminal shapes, blocked moves, mouse dragging, settings/paste, save/cancel, adding a network tile, a missing interface, recovery from a bad config reload, and clean exit.

Visual review sizes: **120×30**, **80×24**, **42×28**, and **38×16**. Additional layout/readability checks: **120×12**, **38×40**, and **26×10**, plus tiny windows that display the resize prompt. Full keyboard/mouse runtime checks were performed on Linux. macOS ARM64 and Windows x86-64 were checked with `cargo check --all-targets` against their targets; those checks do not execute the app on those operating systems.

The SVGs in [previews](previews/) are generated from Ratatui's actual rendered cell buffer with fixed sample metrics. They contain no live machine details. Font rendering may differ slightly from your terminal.

## Remaining boundaries

- Native macOS/Windows runtime and terminal-specific mouse behavior need those environments. CI is configured for all three operating systems; the previous GitHub run was blocked by account billing/spending limits.
- Small default profiles show a subset of tiles, as declared in TOML. Editing a different profile while the terminal is small can still yield compact placeholders; enlarge the terminal to preview that layout at its intended size.
- Tile content remains display-only. Grid rules still live in TOML; the UI edits placement and tile settings.
- Adding tiles requires free grid space. No automatic rearrangement is performed.
