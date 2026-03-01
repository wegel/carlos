# carlos

Terminal frontend for `codex app-server`.

## Status

Alpha.

## Features

- `carlos` starts a new thread
- `carlos resume <SESSION_ID>` resumes a thread
- `carlos resume` opens a thread picker TUI
- multiline input (`Shift+Enter` / `Alt+Enter`)
- turn steering while assistant is running
- mouse scroll + drag selection + copy
- OSC52 clipboard fallback over SSH
- markdown/code rendering with syntax highlighting
- diff rendering for tool/file changes

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
cargo run -- resume
cargo run -- resume <SESSION_ID>
```

## Testing

```bash
cargo test
```

## Controls

- `Enter`: send message (or steer while a turn is active)
- `Shift+Enter` / `Alt+Enter`: newline in input
- `Ctrl+C`: quit
- `Ctrl+Y`: copy selection or last assistant message
- `Esc` / `Ctrl+L`: clear selection
- `g/G` or `Home/End`: jump top/bottom (empty input)
- `Up/Down` + `PageUp/PageDown`: transcript scroll (empty input)
- mouse wheel: scroll
- left drag: select
- left release: copy selection

## Notes

- SSH clipboard is OSC52-based
- currently tested mainly on Linux terminals
