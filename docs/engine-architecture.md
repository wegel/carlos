# Engine architecture: daemon and clients

Status: agreed 2026-11-11. ExecPlans 016 through 018 completed 2026-11-13; the owner reviews EP018 before EP019 starts.

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
| `carlos-client` | Client core shared by every client: wire codec, connection steps, local copy of each session, derived display facts. | No socket, no thread, no async runtime, no terminal crate. |
| `carlos` | Terminal UI. A client of the daemon; the first user of `carlos-client`. | Holds view state only. |

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

## Shared client core (decided 2026-11-11)

Every client must decode messages, greet the daemon, subscribe, reconnect,
resume from the last `seq`, apply events to a local copy of the session, and
match a locally echoed message with the daemon's entry. The TUI needs this
logic in Rust anyway. The phone apps and the web app reuse it, so there is one
copy, tested once against the daemon, where four copies would drift apart.

- `carlos-client` is a pure state machine. The platform owns the socket:
  `tungstenite` in the TUI, `URLSessionWebSocketTask` on iOS, OkHttp on
  Android, the browser `WebSocket` on the web.
- The platform passes each received message to the core. The core returns the
  ids of the entries that changed and the messages to send. The UI reads only
  the changed entries, so one call per batch of events crosses the language
  boundary.
- The core also takes the current time as an argument where it needs one
  (reconnect delays), so tests are deterministic.
- UniFFI generates the Swift and Kotlin bindings. The web app compiles the
  crate to WebAssembly or uses a TypeScript copy; decide in the web plan.
- Markdown structure (`pulldown-cmark`) belongs in the core. Syntax colours
  need a larger library; measure the app size before they go in.
- Native code keeps layout, scroll, text wrap, sheets, dictation, device
  settings and notifications.

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

1. Typing and scrolling stay local. Expansion uses bounded reads and optional live output watches.
2. A streaming event carries only the new text.
3. A client shows a sent message at once (local echo, matched later by
   `client_message_id`).
4. Add a timed delta merger only if real socket measurements justify it.
5. Per-client queues are bounded. A slow client never blocks a session thread;
   the daemon drops that client's queue and sends a snapshot.
6. An entry carries the tool label and summary. A client fetches output lines
   when the user expands the row.
7. A client receives the newest page first and asks for older pages.
8. The daemon parses each backend message once into typed events.
9. `TCP_NODELAY` on. Compression off.

Escape hatch: wire types are serde structs, so a binary encoding can replace
JSON without a protocol change if a profile shows the need.

## Wire protocol, draft 0 (historical)

ExecPlan 018 introduced provisional version 1 in `docs/client-protocol-v1.md` and `crates/carlos-proto`.
Those sources define the current contract. The sketch below records the earlier proposal;
version 1 uses full fact objects, separate live cards, decimal-string cursors and explicit
card ordering rather than ambiguous nullable patches.

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
| 3 | 018 | The core emits sequenced events; the TUI consumes them in-process. Introduce provisional protocol version 1. Delete the old path in a separate commit. |
| 4 | 019 | Session manager, `carlosd`, Unix socket; `carlos-client`; the TUI becomes a client through it; turns survive a TUI quit. |
| 5 | 020 | WebSocket listener, tokens, JSON Schema output, `perf-session` replay through the socket. |
| 6 | 021 | Paged history, Grok approvals, tool summaries as data. |
| 7 | 022 | UniFFI bindings and iOS packaging of `carlos-client`; iPhone app. It compiles on the remote Mac; see `docs/ios-build.md`. |

Only the next ExecPlan is written in full. Each later plan is written when the
plan before it ends, because its details depend on what that plan finds.

## Rust toolchain

Decision (owner, 2026-11-11): use Rust 1.100.0 on every build machine, unless
something fails with it.

State on 2026-11-11: `rustup` cannot download 1.100.0 yet. The stable channel
serves 1.99.1 and the beta channel already serves 1.101.0, so the 1.100.0
release is expected within days. The development machine and the Mac both
have 1.93.0.

To do when 1.100.0 is available:

1. Add `rust-toolchain.toml` with `channel = "1.100.0"` and the components
   `clippy` and `rustfmt`. `rustup` then selects the version on every machine.
2. Pin the same version in `.github/workflows/check.yml` and `release.yml`
   (both use `stable` today).
3. On the Mac, install 1.100.0 as an additional toolchain with the targets
   `x86_64-apple-ios`, `aarch64-apple-ios` and `aarch64-apple-ios-sim`. Do not
   change the Mac's default toolchain; other projects use it.
4. Run `cargo test`, `bash scripts/test.sh` and the `perf-session` baseline
   again, and record new baseline figures, because a compiler change moves them.

Linux/CI update, 2026-11-13: `rust-toolchain.toml` and both workflows now select
Rust 1.100.0. EP017 passed its checks with that compiler. EP017 did not inspect
the Mac; check its additional toolchain and iOS targets before the iPhone build,
while leaving its default toolchain unchanged.

## How the work is driven

Location (owner's decision, updated 2026-11-13): all work for ExecPlans 016 through 022 happens
on branch `iphone-spike` (formerly `daemon-split`), in the separate checkout
`/var/home/wegel/work/perso/carlos-daemon`. The main checkout and `main` stay
on the released carlos so the owner can make quick fixes there. Nothing from
`iphone-spike` goes to `main` or to the remote without the owner's request.
Rebase `iphone-spike` onto `main` when `main` moves. Git ignores `etc/` only on
this branch; `etc/mac-build.env` lives in the `carlos-daemon` checkout.

- Claude plans, splits tasks, runs the gates, reads diffs, commits to `iphone-spike`.
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

EP018 closeout, 2026-11-13: the core now publishes numbered changes and the terminal consumes
them in memory. The old redraw-report path was deleted separately. The owner requested direct
Codex work without subagents, so the report labels its engineering check as direct self-review;
it does not claim an independent reviewer. `.agents/reports/exec018-final-report.md` records
the accepted source, final gates, latency measurements and fixed memory costs. No executable
was installed, merged or pushed. The owner approved EP018a before EP019; see the addendum below.

## Pre-daemon changes agreed in EP018a

The owner approved a broad cleanup after Fable 5.2 reviewed the entire roadmap. A phone must eventually expand running tools, so the protocol supports that workflow before the daemon is built. `docs/client-protocol-v1.md` defines bounded source cursors and separate optional output watches. Compact session facts reach every subscriber without filtered sequence gaps. The original tool body remains the one stored source, preserving terminal colors and copy behavior.

`docs/terminal-client-migration.md` lists the terminal workflows that EP019 must preserve. It supersedes the assumption that the original eleven phone-oriented commands cover the whole terminal. EP019 first proves the daemon with headless clients, then adds typed native-control families and the shared Rust client, then switches the terminal after parity checks. It deletes the embedded path separately. It cannot keep a permanent direct-core bypass or generic backend JSON command.

Backend frames must not inspect a client's composer, confirmation or focused panel. Commands carry their originating operation and explicit read targets; each client decides what to do with its own drafts. Backend calls execute outside the session owner's borrow, then settle only if their connection and operation still match. The daemon can process facts and reads while a worker waits.

Version 1 remains provisional until real remote consumers exercise it. UniFFI stays in EP022 when the iOS consumer exists; JSON Schema and a timed delta merger wait for actual need. The shared-client objective, one-owner-per-session model, bounded queues and per-EP owner review remain in force. EP018a runs directly in Codex without implementation subagents, as requested by the owner.
