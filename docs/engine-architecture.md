# Engine architecture: daemon and clients

Status: agreed 2026-11-11. Work has not started. ExecPlan 016 is the first step.

This note records the design for splitting `carlos` into a daemon and several
clients, and the reasons. Update it when a decision changes. Keep raw logs out.

## Goal

One daemon per host (`carlosd`) runs codex, claude and grok sessions. The
terminal UI, an iPhone app, an Android app, a web app and later a desktop app
connect to it. The phone screen is specified in `tmp/main-screen.md`; section 11
of that file lists what a client needs from the daemon.

Phones and browsers reach the daemon through WireGuard, so the design treats
every client as if it were on the local network.

## Central rule: fast and lean

Carlos must never feel slow or bloated. Every choice below is checked against
that rule. `carlos perf-session` figures recorded before ExecPlan 016 are the
baseline; a regression blocks a commit.

## State of the code before the split (survey of 2026-11-11)

- One binary crate. About 32,000 lines do not reference terminal crates; about
  10,600 lines do.
- `BackendClient` (`src/backend.rs`) abstracts the three backends. It passes
  Codex-shaped JSON strings. No neutral event model exists.
- App code translates Claude events (`src/app/claude_sdk_events.rs`), not the
  Claude backend.
- `AppState` (`src/app/state.rs`) holds session facts, the composer, the
  viewport and render caches in one struct.
- Transcript entries (`Message`, `src/app/models.rs`) have no ids. Maps point
  from backend ids to vector indices (`src/app/state_items.rs`).
- One process holds one session. The backend process dies when the TUI quits.
- Grok approves every request itself and rejects approval replies.
- History loads completely before display.

## Crates

| Crate | Content | Rule |
|---|---|---|
| `carlos-proto` | Wire types (serde). Emits a JSON Schema for Swift, Kotlin and TypeScript clients. | No I/O. |
| `carlos-core` | Backends, session logic, tool labels, Ralph mode. | No terminal crate. The compiler enforces the boundary. |
| `carlosd` | Session manager, Unix socket listener, WebSocket listener. | No terminal crate. |
| `carlos` | Terminal UI. A client of the daemon. | Holds view state only. |

## Session core

- One thread per session. The thread owns the session state and its
  `BackendClient`. It receives client commands, backend events and timers on
  one channel. This keeps the current thread-and-channel model; the core has no
  async runtime.
- The session thread numbers every event it emits (`seq`, per session, starts
  at 1). It keeps recent events in a bounded ring in memory.
- A client subscribes with the last `seq` it received. The daemon replays the
  missing events from the ring. If the ring no longer holds them, the daemon
  sends a snapshot.
- Every entry has a stable id that the daemon assigns. Clients never see
  vector indices or raw backend ids.
- The daemon sends meaning, not layout: Markdown source, tool output lines with
  a line kind, diff counts, phase. Each client wraps and renders for its own
  width. A client decides whether reasoning fits on one row.
- Several clients can watch one session. The first approval decision wins; the
  daemon then sends the resolved approval to every client. Drafts and display
  settings stay on each device.
- The backends keep the conversation files. The daemon adds one SQLite table of
  sessions: id, name, backend, cwd, backend thread id, Ralph state.
- One daemon per host. A client connects to several daemons and groups sessions
  by host. There is no central hub.
- Dictation stays in each client. The TUI keeps in-process Whisper. A phone
  captures its own audio.

## Transport

Decision: WebSocket carrying JSON-RPC 2.0 for network clients; the same
messages over a Unix socket for the TUI.

| Option | Why not, or why |
|---|---|
| WebSocket + JSON-RPC 2.0 | Chosen. One ordered two-way stream. Browsers, iOS and Android support it without extra libraries. Carlos already depends on `tungstenite` and already speaks JSON-RPC to Codex. |
| HTTP POST + server-sent events | Works, but commands and events travel on separate connections, so a client must reconcile their order. |
| Plain TCP, custom framing | A browser cannot open a TCP socket; the web app would need a WebSocket bridge anyway. |
| gRPC | Browsers need a proxy and cannot stream in both directions. |
| QUIC / WebTransport | Keeps a connection across network changes; WireGuard already does that. |

Listener: one thread per connection with `tungstenite`. No `tokio`, no `axum`.
Reason: the daemon serves fewer than ten clients, and a second concurrency
model plus a larger binary contradict the central rule. The protocol does not
depend on this choice.

The daemon runs shell commands as the user. It binds to a configured address
(the WireGuard interface) and requires a token from each device.

## Speed rules for the protocol

1. Typing, scrolling and row expansion never cross the socket.
2. A streaming event carries only the new text.
3. A client shows a sent message at once (local echo, matched later by
   `client_message_id`).
4. For network clients the daemon merges text deltas into one message per
   16 ms per client.
5. Per-client queues are bounded. A slow client never blocks a session thread;
   the daemon drops that client's queue and sends a snapshot.
6. An entry carries the tool label and summary. A client fetches output lines
   when the user expands the row.
7. A client receives the newest page first and asks for older pages.
8. The daemon parses each backend message once into typed events.
9. `TCP_NODELAY` on. Compression off.

Escape hatch: wire types are serde structs, so a binary encoding can replace
JSON without a protocol change if a profile shows the need.

## Wire protocol, draft 0

Draft. ExecPlan 016 refines the entry model; ExecPlan 018 freezes version 1.

Requests, client to daemon:

| Method | Params | Result |
|---|---|---|
| `hello` | `protocol`, `client`, `token` | `protocol`, `host`, `daemon_version` |
| `sessions/watch` | none | `sessions: [SessionInfo]`; then `sessions/changed` notifications |
| `session/create` | `backend`, `cwd`, `name?` | `SessionInfo` |
| `session/subscribe` | `session`, `since?` | replayed events, or `Snapshot` |
| `session/unsubscribe` | `session` | none |
| `session/history` | `session`, `before` (entry id), `limit` | `entries`, `has_older` |
| `session/send` | `session`, `text`, `client_message_id` | `entry` id |
| `session/interrupt` | `session` | none |
| `session/approve` | `session`, `approval`, `decision` (`once`, `session`, `deny`) | none |
| `session/setRalph` | `session`, `on` | none |
| `entry/output` | `session`, `entry` | `lines: [{kind, text}]` |

Notifications, daemon to client:

- `sessions/changed`: `session: SessionInfo`, or `removed: id`.
- `session/event`: `session`, `seq`, `event`. Event kinds:
  `entry_added {entry}`, `text_appended {entry, text}`,
  `entry_updated {entry, patch}`, `approval_resolved {entry, resolution}`,
  `info_changed {patch}`, `reset {snapshot}`.

Types:

- `SessionInfo`: `id`, `name`, `backend`, `model`, `host`, `cwd`, `phase`
  (`idle`, `running`, `approval`), `ralph`, `turn_started_at`,
  `context {used, max}`, `quota? {remaining_pct, days_to_reset}`.
- `Entry`: `id`, `kind` (`user`, `reasoning`, `commentary`, `answer`, `tool`,
  `approval`, `notice`), `text`, `done`, plus per kind:
  - tool: `tool_kind` (`command`, `read`, `search`, `edit`), `label`,
    `label_source` (`literal`, `ai`), `status` (`running`, `ok`, `failed`,
    `interrupted`), `summary`, `added`, `removed`, `has_output`.
  - approval: `command`, `reason`, `choices`, `resolution?`.
- `Snapshot`: `info`, `entries` (newest page), `seq`, `has_older`.

## Order of work

| Step | ExecPlan | Content |
|---|---|---|
| 1 | 016 | Stable entry ids. Split `AppState` into session facts and view state. Move Claude translation into the Claude backend. No behaviour change. |
| 2 | 017 | Cargo workspace; `carlos-core` compiles without terminal crates. |
| 3 | 018 | The core emits sequenced events; the TUI consumes them in-process. Freeze protocol version 1. Delete the old path in a separate commit. |
| 4 | 019 | Session manager, `carlosd`, Unix socket; the TUI becomes a client; turns survive a TUI quit. |
| 5 | 020 | WebSocket listener, tokens, JSON Schema output, `perf-session` replay through the socket. |
| 6 | 021 | Paged history, Grok approvals, tool summaries as data. |
| 7 | 022 | iPhone app. |

Only the next ExecPlan is written in full. Each later plan is written when the
plan before it ends, because its details depend on what that plan finds.

## How the work is driven

- Claude plans, splits tasks, runs the gates, reads diffs, commits to `main`.
- Codex (`gpt-6.1-sol`, high effort) implements and reviews, through direct
  `codex exec` calls. The Codex plugin sandbox cannot start in the development
  container, so workers run with the access mode from `~/.codex/config.toml`.
- Gates per task: `cargo test`, `bash scripts/test.sh`, `carlos perf-session`
  against the baseline. A fresh Codex run reviews with
  `.agents/reviewers/engineering_reviewer.md`.
- The human reviews after each ExecPlan. Nobody installs the binary to
  `~/.local/bin/carlos` before that review.

## Open items

- How a phone pairs with a daemon and receives its token.
- How the user picks host and cwd for a new session.
- Notifications to a suspended phone (approval needed). They need a push
  service on the public internet.
- Dictation on the phone: on-device recognition, or audio sent to the daemon.
