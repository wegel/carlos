# carlos

Rust TUI frontend for `codex app-server`, implemented as a separate app from the Zig version.

## Features

- `carlos` starts a new thread
- `carlos resume <session-id>` resumes a specific thread
- `carlos resume` opens a session picker TUI
- Multiline input via `ratatui-textarea` (input grows with content)
- `Enter` sends turn, or steers when a turn is active
- `Shift+Enter` inserts a newline
- Mouse wheel scroll in transcript
- Drag selection + release-to-copy
- `Ctrl+Y` copies current selection, or last assistant message when no selection
- Selection/copy is padding-aware and Unicode cell-aware (wide chars / emojis)
- Markdown fence delimiter lines (``` ) are hidden in transcript
- Help modal (`?` / `Esc`) blocks normal input while open

## Build

```bash
cargo build
```

## Run

```bash
cargo run
cargo run -- resume
cargo run -- resume <SESSION_ID>
```

## Test

```bash
cargo test
```

## Controls

- `Enter`: send/steer
- `Shift+Enter`: newline in input
- `Ctrl+C`: quit
- `Ctrl+Y`: copy selection or last assistant message
- `Esc` / `Ctrl+L`: clear selection
- `?`: open/close help (help is modal)
- `g/G` or `Home/End`: jump top/bottom (when input is empty)
- `Up/Down`: edit input or scroll transcript when input is empty
- `PageUp/PageDown`: scroll transcript
- Mouse wheel: scroll
- Left drag in transcript: select
- Left release after drag: copy
