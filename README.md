# carlos

Terminal frontend for `codex app-server`.

## status

Alpha.

## features

- start a new codex thread with `carlos`
- resume with `carlos resume <SESSION_ID>` or pick from `carlos resume`
- multiline input with `Shift+Enter` / `Alt+Enter`
- shell-like input history navigation with `Up/Down`
- rewind mode for prompt replay/edit (`Esc,Esc` on empty input)
- turn interrupt while agent is running (`Esc`)
- markdown rendering and code syntax highlighting
- diff rendering with hunk-oriented display
- compact tool/action rows (`Read`, `Search`, `Edit`, `Diff`, `run ...`)
- mouse scroll and drag selection with auto-copy on release
- OSC52 clipboard support for SSH sessions
- context usage indicator (`used/max (%)`) on the activity line
- context compaction markers in transcript

## build

```bash
cargo build --release
```

## run

```bash
cargo run
cargo run -- resume
cargo run -- resume <SESSION_ID>
```

## test

```bash
cargo test
```

## controls

- `Enter`: send message (or steer while a turn is active)
- `Shift+Enter` / `Alt+Enter`: newline in input
- `Up/Down`: input history navigation
- `Esc` (while turn active): interrupt running turn
- `Esc,Esc` (idle + input non-empty): clear input
- `Esc,Esc` (idle + input empty): enter rewind mode
- rewind mode `Up/Down`: select prior user prompts (also repositions transcript)
- rewind mode `Enter`: send selected/edited prompt
- rewind mode `Esc`: leave rewind mode and restore current draft
- `Ctrl+Y`: copy selection or last assistant message
- `Ctrl+L`: clear selection
- `PageUp/PageDown`: transcript scroll
- `g/G` or `Home/End`: jump top/bottom (empty input)
- mouse wheel: scroll
- left drag: select
- left release: copy selection
- `Ctrl+C`: quit

## notes

- SSH clipboard uses OSC52
- currently tested mainly on Linux terminals
- optional perf overlay/report: `CARLOS_METRICS=1`
