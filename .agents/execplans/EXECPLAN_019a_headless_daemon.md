# EP019a: prove a daemon with real backends and headless clients


This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. The owner accepted EP018a and asked to split EP019 after Fable 5.2 medium reviewed the whole roadmap. This document prepares the next execution; no daemon code was written while authoring it.

## Purpose / Big Picture


A daemon is a process that keeps running after its clients disconnect. After this step, a small program without a terminal screen can create a Carlos session over a private Unix socket, send text, watch a running tool, reconnect, and answer an approval. Another client can read the session or interrupt work while a send waits for the native backend. Closing the client leaves the session and backend alive.

The existing terminal remains embedded during this step: it still owns its session in process. EP019b will move all terminal workflows through the daemon and remove that path. This deliberate intermediate state ends at an owner review gate; it is not a second permanent terminal mode. This step does not build an iPhone app, network listener, language bindings, generic backend passthrough or every native terminal panel.

## Progress


- [x] (2026-11-13) Confirm EP018a is archived unchanged, record owner acceptance and preserve the independent follow-up review.
- [x] (2026-11-13) Write EP019a and scope EP019b in the architecture and terminal migration documents.
- [ ] Capture starting executable/source evidence and prove real backend command handles can run without blocking their owner; pass the mandatory gate before listener work.
- [ ] Classify core reports, unify operation ids and implement a bounded owner loop with ordinary-command order and responsive guarded controls.
- [ ] Add carlosd, a minimal saved-session registry and a private Unix listener for the existing basic command families.
- [ ] Prove two headless clients, disconnect/reconnect, bounded events/output, approval races, retry handling and shutdown through real deterministic adapters.
- [ ] Pass feature, terminal, native contract, dependency and speed checks; review, report and move this completed plan unchanged to done.
- [ ] Stop for owner review before writing the full EP019b from the working daemon and its observed limits.

## Surprises & Discoveries


EP018a already supplies BackendWork values with no SessionState borrow, but its delayed-worker test constructs a fake Peer on the worker thread. The real ClaudeClient still holds a RefCell event handler and clients retain receiver fields. A mutex around the whole client would make an interrupt wait behind a blocked send. Source: `crates/carlos-core/src/backend.rs`, `claude_backend/mod.rs`, `codex_connection.rs`, and `session/command_work_tests.rs`.

The core still returns BackendUpdate, ClaudeUpdate, TurnUpdate, CommonUpdate and CodexClosed reports. Some fields describe shared facts, some describe one submitted command, and some tell the terminal to change its controls. SubmissionOutcome has no socket notification yet. The protocol also has both client_message_id and ClientOperation, while Entry.client_message_id remains null. These are known producer/consumer gaps, not an established deployed contract.

The core's one pending ordinary mutation is a useful order guard. The daemon must queue commands above it rather than returning Busy for every second client. Interrupts and approval answers need prompt service and separate validity checks; priority does not authorize an obsolete turn or a second answer to the same card.

## Decision Log


Decision: split the old EP019 into EP019a, the headless daemon proof, and EP019b, terminal protocol coverage, shared client, cutover and deletion. Rationale: each ends in a working behavior the owner can review. Only the next ExecPlan is written in full; EP019b is scoped in `docs/terminal-client-migration.md` until this plan produces real interfaces. Date/Author: 2026-11-13, owner/Codex.

Decision: make real adapter concurrency the first gate, before sockets. Rationale: a working fake cannot prove a real Claude or Codex connection remains interruptible. Do not accept a lock held across an RPC reply wait as a solution. Date/Author: 2026-11-13, Codex.

Decision: keep the core mutation fence, add bounded FIFO admission above it, and handle interrupt/approval controls independently with their own turn/card/connection checks. FIFO means ordinary commands keep their arrival order. Already accepted native prompt queues remain backend-owned. Date/Author: 2026-11-13, Codex.

Decision: use ClientOperation as the sole semantic command identifier, with the daemon supplying the client identity. JSON-RPC request ids still pair one envelope with its reply, but are not send-retry identities. Before remote sends exist, replace random operation UUIDs with a per-client increasing, lossless decimal counter; SessionId and EntryId stay opaque. Rationale: a high-water mark lets the server reject expired retries without an unbounded UUID tombstone set. There are no deployed remote clients requiring compatibility code. Date/Author: 2026-11-13, Codex.

Decision: expose a fresh daemon-instance id in the greeting and retain logical session ids in the small registry. A new instance requires fresh snapshots and explicit recovery of uncertain commands, not automatic resend. Rationale: in-memory sequence/retry records do not survive a daemon restart. Do not claim exactly-once execution across a crash. Date/Author: 2026-11-13, Codex.

Decision: list saved sessions without starting their backends, then restore a dormant session once when a client subscribes. Rationale: the daemon must recover a saved logical id without launching every historical helper or creating duplicate helpers for concurrent subscribers. Save Ralph state for display/review, but require an explicit decision to resume it after a crash. Date/Author: 2026-11-13, Codex.

## Outcomes & Retrospective


The author wrote the plan; no daemon implementation has started. The next action is to preserve the EP018a executable and build the real-adapter proof. EP019b remains a scoped follow-up, not an executable stub. The terminal cutover and removal of its legacy reports are not completion criteria for EP019a.

## Context and Orientation


Work from `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`. Do not change the main checkout, merge, push or install a spike binary. Rebase this branch when main advances and rerun the affected checks. The owner still reviews each completed ExecPlan.

The workspace has the terminal package at the repository root, `crates/carlos-core` for session/backend code, and serde-only `crates/carlos-proto` for shared data. The terminal owns AppState with SessionState and ViewState. Core SessionState stores the original transcript; publish returns a borrowed batch whose lifetime prevents a write during encoding. The owner numbers shared events in order with a per-session sequence. An output cursor is an opaque token that lets a client resume a tool body independently; clients that keep tools collapsed receive only small change facts. A backend adapter is the Rust code that speaks a native backend's protocol. A command handle lets a worker send requests without owning the adapter's event receiver. A private result describes one client's command, while a shared fact describes the session for every subscriber.

`crates/carlos-core/src/session/command_work.rs` executes prepared send/fork/name data; `command_guard.rs` rejects stale completions. `turn_interrupt.rs`, `backend_actions.rs`, callback replies, settings, lifecycle work and some file reads still contain I/O beside state changes. Existing token/deadline-based asynchronous reads need not be converted into another work framework. `crates/carlos-core/src/claude_backend/client_prompt.rs` and `sessions.rs` can switch native helpers; `codex_connection_backend.rs` and `codex_connection.rs` can replace native connections. A generation identifies one such connection. An epoch distinguishes conversation changes within it. A worker must capture and validate both before writing, not merely discard its reply afterward.

Existing real-process fixtures live under `crates/carlos-core/src/tests/`, `claude_backend/*tests.rs`, `grok_backend/receiver_test_support.rs`, `tests/ui/fake_codex.py` and the native Claude controller fixtures. They drive real adapter code through pipes or sockets without inference or an account. Reuse them instead of mocking BackendClient for the first gate. The completed EP018a report records 816 app, 250 core and six protocol tests, three compiler checks, 177 terminal workflows and 23 native-monitor workflows.

## Plan of Work


### Milestone 1: prove real command handles before a listener exists


Refactor `crates/carlos-core/src/backend.rs` and the concrete Codex, Claude and Grok clients so the session owner holds the receiver and mutable native event normalizer, while workers receive a small shareable command handle. Put that interface in `crates/carlos-core/src/backend_commands.rs`. Retain one backend implementation: the embedded terminal may use an adapter over the split pieces until EP019b, but cannot have a separate prompt/reply algorithm.

A command handle must refer to a captured native connection and conversation. Cloning it must not silently follow a later helper replacement. Read short-lived shared metadata under a short lock where necessary; never hold the normalizer, SessionState or a client-wide lock while waiting for a reply. Retired work that has not begun must fail before sending. Work already written when a reset occurs remains uncertain and cannot be retried automatically.

Add real-adapter tests under `crates/carlos-core/src/tests/backend_owner_tests.rs` and backend-specific support files as needed. Withhold a send reply in a deterministic peer, then observe an owner-applied backend fact and another client's read while it waits. The peer must also observe an interrupt and a valid callback answer before releasing that send reply. Assert causal order using channels/barriers, not a sleep-based timing guess. Test reset before dispatch, helper replacement, late reply, duplicate native completion, EOF and blocked stdin. Existing write limits and timeouts remain bounded. Codex and Claude must pass this proof; adapt and verify Grok before exposing its daemon create path, preserving its current capability limits.

This milestone is a gate: do not build a listener around a fake-only proof or a client-wide mutex. Fix the owner/handle split until the real adapters pass. Use AGENTS.md's hard-blocking protocol only if an actual external decision or invariant prevents a correct implementation. Run cargo test and the existing terminal/native contract runner after the adapter change.

### Milestone 2: own facts and schedule commands in one place


Add a terminal-free owner module under `crates/carlos-core/src/engine/`. One thread owns each SessionState, receives commands, native facts, timer wakeups and worker completions, and publishes checked changes. Lifecycle work, disk reads and native waits execute outside that state borrow. A narrow typed EngineCommand/EngineReply facade is the daemon's route; it must not expose arbitrary native method names or generic session setters.

Before adding dispatch cases, write `docs/core-report-routing.md`. Account for every field of the current report families and related native-control reports. Assign each to a numbered session fact, a private operation result retained for its owner, or client-local behavior that is deleted from the daemon path. Implement the routes needed by the basic commands now and explicitly assign the remaining native panel families to EP019b. A client connecting after a reset must recover shared state from a snapshot; it cannot need a lost synchronous return value. Unsaved composers never enter the core. Private recovery cannot depend on another client's editor or global panel selection.

Queue ordinary mutations in arrival order with count and byte limits. The core's Busy result stays internal until its current slot is free. A full admission queue returns an explicit not-admitted result before dispatch; the originating client keeps its text. Interrupts and approval answers use a separate bounded control lane and cannot wait behind an ordinary send's reply. They still verify session/connection generation, the exact target turn or live card, offered choices, submitted state and native capability. Claim a card atomically on the owner so two clients cannot both send a reply. A timeout after a possibly written answer is uncertain, not permission for another client to answer again. Definite pre-write failure can reopen it; native withdrawal retires it. Add a narrow shared retry/recovery policy rather than duplicating one in each connection thread.

Use ClientOperation for deduplication and echoed entry ownership, removing the redundant client_message_id contract. Change its operation id to a monotonically increasing decimal counter per registered client, and update the embedded terminal's allocator/tests together. The daemon issues a client identity at greeting; a reconnect may resume a still-retained identity, but an unknown/expired identity must not be recreated with its old retry rights. Keep bounded pending/recent result records and an admission high-water mark for each retained client. An identical retry attaches to its pending result or returns the same completed result without executing again. Reusing an id for different work fails. An older id outside the retained result window returns outcome-expired/uncertain, never a new execution. A new daemon instance or expired client lease requires fresh state and explicit user recovery. Bound retained inactive client identities as well as their results. Record the chosen limits and all expiry behavior in the protocol documentation and tests.

Add typed private operation notifications to carlos-proto for the implemented outcomes, with a resume/query route using the same operation identity. A socket disconnect must not consume and lose a recovery result. Unknown future report types fail visibly rather than silently disappearing. The old terminal reports may remain only as the explicitly scheduled EP019b bridge; never serialize ClaudeEffect or TurnUpdate as the public wire protocol.

### Milestone 3: run carlosd and serve the basic local commands


Add `crates/carlosd` to workspace members and default-members, with a library for process tests and a `carlosd` executable. Use standard threads/channels, existing serde types and rusqlite. Keep daemon code free of terminal, audio and speech dependencies, and extend `scripts/check-core-dependencies.py` to check its resolved graph. No async runtime or WebSocket listener belongs here.

Provide `carlosd --socket PATH --state-dir PATH`. Bind a Unix socket in a private directory for the current OS user and reject unsafe paths/other-user access. Refuse to unlink another live daemon's socket. Bind no TCP address. Network tokens and pairing stay in EP020; the local greeting establishes protocol version, daemon instance and client identity. Use newline-delimited JSON-RPC, matching the existing message envelopes. Add a small standard-library Python driver under `tests/daemon/` to exercise it; it is test tooling, not a second production client library.

The manager owns session launch/stop and a small SQLite catalogue containing Carlos id, name, backend, cwd, native thread id and saved Ralph state. On startup, load saved session descriptions without launching every backend. Subscribing to a dormant saved session restores it once; concurrent subscribers share that launch and receive a snapshot after history adoption. A failed restore leaves a visible unavailable description and returns an explicit error, not a duplicate new session. session/create creates a new logical session rather than guessing a saved one. Do not persist event history or add a job database. Creation/restore runs off the session owner; a failed launch returns a typed result without leaving a false running session. Logical catalogue ids survive daemon restart. The new daemon instance invalidates old event/retry cursors. Restore native history before publishing a fresh snapshot; do not replay uncertain input or automatically restart Ralph after a daemon crash. While the daemon remains alive, detaching clients must not stop backend work or Ralph. Use only temporary state directories and fake native processes in automated tests.

Implement greeting, sessions watch/create, subscribe/unsubscribe, stored-history pages, send policy, interrupt, live-card answers, Ralph toggles and normalized output reads/watches. Launch all three backends through core factories; preserve explicit unsupported choices such as current Grok approval limits. Expose no generic native passthrough and do not add task/goal/queue/settings/rewind families just to enlarge this milestone. Rename/fork preparation already exists but their remote terminal commands belong to EP019b.

Keep the recent shared-event ring, per-connection outgoing bytes, ordinary/control queues, output watches and saved private outcomes bounded by both counts and bytes. Derive initial budgets from the existing native frame limits and captured synthetic fixtures; document exact constants before accepting socket traffic and test each limit. Publish/encode once per owner batch, and perform socket writes on connection workers. A slow client must not stall another client or the owner. If shared-event replay is unavailable, send a fresh bounded page; if a single source entry cannot fit, use an explicit bounded read or fail visibly without truncating it or killing healthy backend work. Any initial oversized-read limitation must be explicit and resolved before EP019b's terminal cutover. Output chunks use EP018a's separate cursors, with no filtered session-event gaps. On output-queue overflow require an explicit fresh watch instead of skipping bytes. Add only the scheduling needed by these headless consumers, without a timed merger.

### Milestone 4: prove the whole path with two headless clients


Add `tests/daemon/test_daemon.py`, runnable against an explicit daemon executable, using a temporary socket/state directory and deterministic native peers. Test creation, live text, a long running tool with one expanded watcher and one collapsed watcher, reconnect at both cursors, history pages and session-list changes. The two clients must receive the same numbered shared facts while keeping their private operation results separate. Hold a native send reply, accept a second client's queued text, then observe a read, an interrupt and a permitted answer before the send completes. Check that the ordinary queued text is dispatched once in the original order after the slot opens.

Test stale/foreign ids, unsupported answers, malformed frames, duplicate operation ids with equal/different payloads, expired results, reconnect with pending work, a daemon restart, bounded queue overflow, worker failure and shutdown. Race two answers to the same card and verify exactly one native reply. After one headless client disconnects, another must still see the same live session and backend process. Explicit daemon shutdown stops only processes it owns and leaves attached native servers alone. No test may call a user model or read an unrelated saved session.

The current terminal continues using its embedded path in this step, but all its existing workflows must still pass after the adapter/owner refactor. Compare carlos perf-session with preserved EP018a and the original architecture reference. Add a headless replay measurement using the same synthetic/captured inputs; report owner apply cost separately from encoding/socket/client cost, plus bytes and resident memory. Do not silently compare a different feature build or worktree target with a cached executable.

### Milestone 5: close the headless step for owner review


Run the applicable reviewer prompt, resolve blocking findings, and record final source/compiler/binary identities, process-test artifacts and measurements. Follow the owner's direct-work/no-implementation-subagent preference; label a direct engineering self-review honestly. Move the completed plan unchanged to `.agents/done/`. Stop at owner review. Then write EP019b from the real facade and documented remaining gaps; do not automatically start terminal migration.

## Concrete Steps


Run shell commands through rtk from `/var/home/wegel/work/perso/carlos-daemon`. First preserve the accepted EP018a executable if its artifact is still present, verify its hash, and capture a fresh baseline before source changes. Use a separate target for any other checkout.

    rtk proxy cargo test
    rtk proxy bash scripts/test.sh --codex-contract

During milestone 1 run the new real-adapter tests by name and retain their native request logs. The peer should show an interrupt/answer before the withheld send reply. After the daemon package and tests exist, the repeatable commands are:

    rtk proxy cargo test --workspace
    rtk proxy cargo test --workspace --no-default-features
    rtk proxy cargo clippy --workspace --all-targets -- -D warnings
    rtk proxy cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy cargo build -p carlosd
    rtk proxy python3 tests/daemon/test_daemon.py --binary target/debug/carlosd --artifacts target/daemon-artifacts/ep019a

For a manual observation run `rtk proxy mktemp -d /tmp/carlos-ep019a.XXXXXX` and keep its returned private directory path. Pass that path with `/socket` and `/state` appended to `rtk proxy target/debug/carlosd --socket PATH/socket --state-dir PATH/state`; replace PATH with the returned directory. The test driver must print the exact greeting/create/subscribe example and session id it uses; never assume the native backend id is the Carlos id. After close/reconnect the same Carlos id must still resolve. Stop the daemon explicitly after the observation.

For final gates run `rtk proxy cargo build --workspace --release --no-default-features`, then `rtk proxy cargo build --workspace --release` to restore default features. Run `rtk proxy bash scripts/test.sh --release --codex-contract`, `rtk proxy python3 tests/daemon/test_daemon.py --binary target/release/carlosd --artifacts target/daemon-artifacts/ep019a-release` and `rtk proxy python3 scripts/check-core-dependencies.py`. Extend scripts/test.sh and CI to run the new tests so passing them is not dependent on this prompt. Use `carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48` and `carlos perf-session src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48`; run quiet paired samples with matching output counts. No spike executable is installed in this plan.

## Validation and Acceptance


The compiler passing does not prove that a waiting backend leaves the owner responsive. Real Codex/Claude adapter logs must prove an owner fact/read, an interrupt and a valid callback answer can advance before a held send reply. Reset-before-dispatch must produce no stale prompt on a replacement helper; an already written command's late reply cannot change the new session or revive a completed turn. The same mechanism must support Grok's current basic capabilities without breaking its terminal tests.

Two headless socket clients must watch one surviving session, resume shared events and tool output independently, submit ordered ordinary text once, receive only their own private recovery, and race one approval with exactly one native answer. The owner must keep servicing facts and reads under full/slow connection queues. No unbounded replay log, retained client set, output watch set or result ledger is accepted. Explicit expired/unsupported errors preserve text and never imply that uncertain native work was rejected. A daemon restart is distinguishable from a live reconnect.

All terminal workflows, feature/lint/compiler/dependency/native checks pass. Measured terminal latency must meet the original architecture baseline and be compared with EP018a. Record socket costs honestly rather than attributing native apply time to the UI. Successful closeout leaves the old terminal route only because EP019b is an explicit reviewed follow-up, not because missing remote features were waived.

## Idempotence and Recovery


Use temporary sockets/state/peers for tests. Repeating a test must not reuse a user session or terminate an unrelated process. Refuse a second daemon at a live socket; remove only a stale socket whose ownership is proven. Keep buildable commits for the real-handle split, owner/report routes, basic daemon, headless tests and closeout. Do not rewrite completed plans. Never automatically replay a possibly written prompt, answer or mutation after timeout, reconnect or daemon restart. If a prototype is discarded, remove it before promoting the chosen path; do not leave multiple backend execution implementations.

## Artifacts and Notes


Accepted EP018a runtime source is `43b34609eee2912077b7fbecb70de5025773036a`; closeout is `51c4a00`. Its default release SHA256 is `3b7970cae7f4d33cc1687715ba6990e36315482324f8e015cc72fcff04c262be`, 20,188,600 bytes. The original architecture reference SHA256 is `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`. Prior samples live under `/tmp/carlos-ep018a/` when retained; the tracked report is `.agents/reports/exec018a-final-report.md`. Capture new evidence if temporary files are gone, rather than inventing baseline results.

Fable's read-only follow-up and verified model/effort metadata are `.agents/reports/exec018a-fable-followup-review.md` and `.json`. The owner agreed to the split and accepted EP018a on 2026-11-13. This is not installation or merge approval.

## Interfaces and Dependencies


In `crates/carlos-core/src/backend_commands.rs`, expose a command-only `BackendCommands: Send + Sync` interface and a concrete captured handle whose connection identity cannot change under it. Keep the event receiver/normalizer in an owner-held backend session type. Adapt BackendWork::execute to the command handle instead of requiring the whole BackendClient. Use typed prepared interrupt/approval work with separate turn/card admission; do not force every token-based read through the ordinary mutation slot.

In `crates/carlos-core/src/engine/`, expose a narrow EngineCommand/EngineReply facade and SessionOwner input/output types. The owner accepts plain typed commands, native facts and checked completions; it never accepts a widget or arbitrary native JSON method. In carlos-proto add the greeting instance/client identity, unified monotonic operation identity and private outcome/resume data needed by actual handlers. Keep the crate serde-only. In `crates/carlosd`, keep socket I/O, catalogue and connection queues outside core ownership. Reuse existing locked dependencies; add no async runtime, schema generator or language binding. Record exact promoted signatures and budgets as the first working prototype proves them.

## Engineering Review


Planning review: the author checked the plan against the repository's code, Fable's whole-roadmap concerns and the owner's qualifications about Busy and private results. The author checked roadmap references and whitespace and verified that the archived EP018a plan stayed byte-for-byte unchanged. This pass changes only documents, so it does not rerun Rust/runtime tests or claim a concurrency proof. During execution use `.agents/reviewers/engineering_reviewer.md`; copy its findings, corrective guidance and final verdict here. No independent reviewer is claimed for this planning pass.

Update note (2026-11-13): created the next executable plan after owner acceptance of EP018a. Split headless daemon proof from terminal cutover; require real handles first, explicit report routing, bounded ordered admission, private recovery and an owner review gate before EP019b.
