# carlos

Terminal frontend for `codex app-server`.

## Status

Alpha.

## Run

```bash
cargo run
cargo run -- resume
cargo run -- resume <SESSION_ID>
```

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Notes

- `Enter`: send message (or steer while a turn is active)
- `Shift+Enter` / `Alt+Enter`: newline in input
- Mouse wheel + drag selection + copy supported
- SSH clipboard uses OSC52
- `carlos resume` opens a session picker
