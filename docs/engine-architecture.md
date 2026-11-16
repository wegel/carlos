# Engine architecture: daemon and clients

Status: EP016 through EP021a are complete and approved. The terminal runs through the daemon;
local/network access and the shared checked panel readers pass their final checks and reviews.
Root approved EP021a under the owner's grant on2026-11-16 at11:34:22 UTC. PROGRAM_PLAN.md
records the exact source/evidence and the separate restrictions on installation, merging
and pushing. EP021b is next; actual UniFFI/iPhone work remains EP022.

This note records the design for splitting `carlos` into a daemon and several
clients, and the reasons. Update it when a decision changes. Keep raw logs out.

## Goal

One daemon per host (`carlosd`) runs codex, claude and grok sessions. The
terminal UI, an iPhone app, an Android app, a web app and later a desktop app
connect to it. The phone screen is specified in `tmp/main-screen.md`; section 11
of that file lists the original phone requirements. EP018a supersedes its running-tool restriction: clients can expand a tool while it runs.

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

- `carlos-proto`: serde wire types with no I/O. Generate schemas when an actual consumer needs them.
- `carlos-core`: backends, session facts, tool labels and Ralph. Cargo forbids terminal dependencies; the compiler protects private facts.
- `carlosd` (EP019a): session manager and Unix listener; WebSocket access follows in EP020. No terminal dependency.
- `carlos-client` (EP019b): pure wire codec, connection steps and local session facts shared by clients. No sockets, threads, async runtime or terminal dependency.
- `carlos`: terminal client with its own composer, widgets, render cache and dictation. It connects to carlosd over a Unix socket and owns no backend or mutable core session.

## Session core

- One thread per session owns SessionState and the backend event receiver/normalizer.
  Workers use command-only handles bound to the captured native connection; no worker
  holds the entire client locked while waiting. The owner receives commands, native
  facts, timers and checked completions. EP019a proves this with real adapters before
  adding a listener. The core keeps threads and channels, without an async runtime.
- The session thread numbers every event it emits (`seq`, per session, starts
  at 1). It keeps recent events in a bounded ring in memory.
- A client subscribes with the last `seq` it received. The daemon replays the
  missing events from the ring. If the ring no longer holds them, the daemon
  sends a snapshot.
- Every entry has a stable id that the daemon assigns. Clients never see
  vector indices or raw backend ids.
- The daemon sends Markdown source, tool output pieces with a line kind, diff counts
  and phase. Each client wraps and renders for its own width. EP019a uses normalized
  output for headless clients. Before terminal cutover, EP019b must also preserve
  original terminal color and copy behavior through bounded source reads from the
  same stored body; it cannot reconstruct lost source from normalized pieces.
  A client decides whether reasoning fits on one row.
- Several clients can watch one session. The first approval decision wins; the
  daemon then sends the resolved approval to every client. Drafts and display
  settings stay on each device.
- The backends keep the conversation files. The daemon adds one SQLite table of
  sessions: id, name, backend, cwd, backend thread id, Ralph state. Logical session
  ids survive a restart, but a new daemon-instance id invalidates event/retry cursors.
  Adopt native history before a Ready snapshot; expose starting facts/cards while it loads. Never automatically resend uncertain
  commands or restart Ralph after a daemon crash. Detaching from a live daemon does
  not stop Ralph.
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

## First phone binding

EP022 binds the Rust calls needed by the specified first iPhone screens. The starting list is greeting/reconnect, session listing and lifecycle, transcript/history/source/output, prompts, interrupts, approvals, context display and Ralph. It is not the full list of public carlos-client methods. The terminal uses the shared queue, host context-default, goal and native task-panel readers now; keep them Rust-only until a phone screen needs them. Do not create Swift mirror records or sheets for those families merely because Rust exposes them. A later web binding follows its actual screens too.

Choose the phone's cache limit, typed capacity notice and fatal client-error handling against its real facade and measured memory. Do not add those speculative knobs to the current terminal/shared reader code. EP021b records production lines added and deleted for each native feature and removes replaced paths. A line count is review evidence, never permission to discard supported data or weaken correctness.

## Transport

Decision: WebSocket carrying JSON-RPC 2.0 for network clients; the same
messages over a Unix socket for the TUI.

- WebSocket plus JSON-RPC 2.0 is chosen: one ordered two-way stream supported by browsers and phones. Carlos already uses tungstenite and JSON-RPC for native Codex.
- HTTP POST plus server-sent events separates commands and facts across connections, requiring more ordering checks.
- A custom TCP stream cannot serve a browser without a bridge.
- gRPC needs a browser proxy and cannot stream both directions there.
- QUIC/WebTransport preserves connections across network changes; WireGuard already handles that need here.

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
   its originating ClientOperation). EP019a replaces the provisional duplicate
   `client_message_id` with that one operation identity before remote sends exist.
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

ExecPlan 018 introduced provisional version 1 in `docs/client-protocol-v1.md` and `crates/carlos-proto`. EP019a implements the local listener and records the actual retry/watch/budget rules in `docs/daemon-protocol.md`.
Those sources define the current contract. The sketch below records the earlier proposal;
version 1 uses full fact objects, separate live cards, decimal-string cursors and explicit
card ordering rather than ambiguous nullable patches.

Requests, client to daemon:

- `hello`: `protocol`, `client`, `token` → `protocol`, `host`, `daemon_version`
- `sessions/watch`: none → `sessions: [SessionInfo]`; then `sessions/changed` notifications
- `session/create`: `backend`, `cwd`, `name?` → `SessionInfo`
- `session/subscribe`: `session`, `since?` → replayed events, or `Snapshot`
- `session/unsubscribe`: `session` → none
- `session/history`: `session`, `before` (entry id), `limit` → `entries`, `has_older`
- `session/send`: `session`, `text`, `client_message_id` → `entry` id
- `session/interrupt`: `session` → none
- `session/approve`: `session`, `approval`, `decision` (`once`, `session`, `deny`) → none
- `session/setRalph`: `session`, `on` → none
- `entry/output`: `session`, `entry` → `lines: [{kind, text}]`

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
    `label_source` (`fallback`, `literal`, `ai`), `status` (`running`, `ok`, `failed`,
    `interrupted`), `summary`, `added`, `removed`, `has_output`.
  - approval: `command`, `reason`, `choices`, `resolution?`.
- `Snapshot`: `info`, `entries` (newest page), `seq`, `has_older`.

## Order of work

1. EP016: stable entry ids, separate SessionState/ViewState, and Claude translation in its backend. Complete.
2. EP017: Cargo workspace and a terminal-free carlos-core. Complete.
3. EP018: numbered core events consumed in process, provisional protocol types and separate removal of the old redraw-report path. Complete.
4. EP018a: bounded running output, client-owned drafts, prepared backend waits, terminal workflow inventory and reduced guards. Complete and accepted by the owner.
5. EP019a, complete and accepted by the owner: real backend handles and a headless daemon with basic commands and two concurrent clients. Sessions survive client disconnect. Tests prove bounded queues, ordered commands, priority controls and private recovery. Archived plan: `.agents/done/EXECPLAN_019a_headless_daemon.md`; evidence: `.agents/reports/exec019a-final-report.md`.
6. EP019b, complete and approved under delegated authority: typed terminal controls and pure carlos-client preserve source/color/copy and every terminal workflow through the daemon. Commit cc2a5f5 separately deletes the embedded path. Final evidence and whole-roadmap review are in `.agents/reports/exec019b-final-report.md`; archived plan: `.agents/done/EXECPLAN_019b_terminal_client.md`.
7. EP020, complete and approved under delegated authority: explicit stop/reopen releases owner slots while preserving saved conversations/private results; opt-in authenticated WebSockets share the same manager. Final source6a31fd059ad6f8f0e7e92b1f001bc765 passes complete gates and matched measurements. Both final Fable reviews and all remaining limits are in `.agents/reports/exec020-final-report.md`.
8. EP021a: share panel read/paging rules in carlos-client, remove terminal copies and make daemon command admission exhaustive. Add the non-blocking partial-progress stall test and truthful write-return diagnostic label. Complete: `.agents/done/EXECPLAN_021a_shared_panel_reads.md`.
9. EP021b: paged native history, Grok approvals and richer tool summaries as data, after the common readers are checked. Next: write `.agents/execplans/EXECPLAN_021b_native_phone_data.md` in full before code.
10. EP022: UniFFI bindings, iOS packaging and the iPhone app. Compile on the remote Mac; see `docs/ios-build.md`.

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
- On 2026-11-14, the owner delegated approval of completed ExecPlans to Codex and asked it to continue to the next EP while the owner sleeps. Codex must pass the checks, perform the direct engineering review, obtain the owner-requested Fable 5.2 medium review of code quality and the whole roadmap, assess its findings and record each auto-approval before continuing. This supersedes the earlier per-EP human pause. Keep the installed binary unchanged and do not merge or push without the owner’s request.

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

`docs/terminal-client-migration.md` lists the terminal workflows that EP019 must preserve. It supersedes the assumption that the original eleven phone-oriented commands cover the whole terminal. EP019a proves the daemon with headless clients and ends at owner review. EP019b adds typed native-control families and the shared Rust client, then switches the terminal after parity checks. It deletes the embedded path separately before its own owner review. It cannot keep a permanent direct-core bypass or generic backend JSON command.

Backend frames must not inspect a client's composer, confirmation or focused panel. Commands carry their originating operation and explicit read targets; each client decides what to do with its own drafts. Backend calls execute outside the session owner's borrow, then settle only if their connection and operation still match. The daemon can process facts and reads while a worker waits.

Version 1 remains provisional until real remote consumers exercise it. UniFFI stays in EP022 when the iOS consumer exists; JSON Schema and a timed delta merger wait for actual need. The shared-client objective, one-owner-per-session model, bounded queues and per-EP owner review remain in force. EP018a runs directly in Codex without implementation subagents, as requested by the owner.

EP018a closeout, 2026-11-13: bounded running output, correlated recovery, prepared send/fork/name waits and the terminal inventory are complete. The source lexer was removed. The branch now includes main's Escape-burst fix. Final feature/release/terminal/native checks and original-reference/common-base measurements pass; `.agents/reports/exec018a-final-report.md` records exact evidence and remaining EP019 work. The owner accepted this step on 2026-11-13. The independent follow-up review is `.agents/reports/exec018a-fable-followup-review.md`; the owner agreed to separate daemon proof from terminal cutover. EP019a is the next execution, with owner review before EP019b. No spike executable was installed, merged or pushed.

EP019a now has the local carlosd executable, fixed native command handles, one owner per session, bounded ordered command admission, independent control workers, authenticated private retry results, encoded shared replay, per-device output watches and a minimal SQLite session catalogue. Codex, the compiled Claude controller and Grok pass deterministic process tests. One shared synthetic generator drives terminal and headless measurements. The terminal remains embedded until EP019b, which must migrate all inventoried workflows and then delete that path. Version 1 remains provisional, the network listener stays in EP020, richer native data stays in EP021, and iPhone code stays in EP022.

Owner direction (2026-11-14): Codex may auto-approve completed EPs and must continue to the next EP rather than pause for human review. The earlier historical review gates above remain as context; this direction governs current execution. Checks, code quality, performance evidence, the separate cutover deletion commit and hard-blocker rules still apply.
