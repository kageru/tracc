# tracc

## Commands
- `cargo run` starts the TUI.
- `cargo build` builds the binary.
- `cargo test` only checks that the crate compiles; there are currently no tests.
- `cargo clippy` runs lint checks; treat warnings as fixes to address before merging.
- `cargo fmt` is the only formatting command in the repo.

## Layout
- Single-crate Rust binary.
- `src/main.rs` sets up the terminal session and calls `Tracc::run()`.
- `src/timesheet.rs` owns day storage and summary logic.
- `src/confirm.rs` and `src/layout.rs` contain the shared TUI widgets.
- `src/tracc/mod.rs` owns the app state and loop.
- `src/tracc/edit.rs`,
  `src/tracc/history.rs`,
  `src/tracc/input.rs`,
  `src/tracc/navigation.rs`,
  and `src/tracc/render.rs`
  split editing, undo/redo, input handling, navigation, and rendering.

## Maintenance
- Update this file whenever the source layout changes.

## Data
- Active timesheets are stored under the OS data directory at `tracc/timesheets/YYYY/MM/DD.json`, not in the repo root.

## Behavior quirks
- Non-today sheets are locked until the user confirms a mutation.
- Undo history is capped at 20 snapshots.
- Time edits accept `HHMM`, `HH:MM`, or plain minutes.
- A trailing `[group]` override changes the summary bucket; `pause`, `lunch`, `mittag`, and `break` all count as `pause`.
- `-` shifts the selected time by five minutes through an internal one-minute adjustment and rounding.
