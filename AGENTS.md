# tracc

## Commands
- `cargo run` starts the TUI.
- `cargo build` builds the binary.
- `cargo test` only checks that the crate compiles; there are currently no tests.
- `cargo fmt` is the only formatting command in the repo.

## Layout
- Single-crate Rust binary.
- `src/main.rs` sets up the terminal session and calls `Tracc::run()`.
- `src/tracc.rs` contains the app loop, keybindings, undo/redo, and persistence.
- `src/timesheet.rs` owns day storage and summary logic.
- `src/layout.rs` and `src/confirm.rs` contain the shared TUI widgets.

## Data
- Active timesheets are stored under the OS data directory at `tracc/timesheets/YYYY/MM/DD.json`, not in the repo root.

## Behavior quirks
- Non-today sheets are locked until the user confirms a mutation.
- Undo history is capped at 20 snapshots.
- Time edits accept `HHMM`, `HH:MM`, or plain minutes.
- A trailing `[group]` override changes the summary bucket; `pause`, `lunch`, `mittag`, and `break` all count as `pause`.
- `-` shifts the selected time by five minutes through an internal one-minute adjustment and rounding.
