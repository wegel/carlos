# Size and design review: `iphone-spike` at 6746fe8 plus the uncommitted resume-target fix

Model: `claude-fable-5-2`. I cannot see the configured effort level from inside the session. Read-only; no builds, tests or edits.

## 1. Verdict

**Some avoidable bloat, concentrated in the native panel families (tasks, queue, settings, goals); the ownership architecture itself is proportionate.** Confidence: medium.

Evidence limits:
- I read about 45 production files end to end across proto, client, daemon, Core engine and terminal, plus the plans and prior Fable reviews.
- I could not diff against `main` (no git access) or open the `/tmp/carlos-ep020` count artifact (permission denied), so the scale figures below are sums of files I read, not measured deltas.
- Most of Core `session/claude`, `session/codex` and the backend adapters is unread.

## 2. Additions justified by the phone/web goal

- **Owner model in Core:** one thread per session, three mailboxes, and captured command handles so interrupt and approve bypass a waiting send (`engine/dispatch.rs:27-102`).
- **Numbered events and replica:** stable entry ids, sequence ring, snapshot fallback, and `Replica`/codec/`Operations` in `carlos-client`.
- **Private retry results:** `retry/mod.rs` is about 325 lines with three reserved classes; it is small for what it guarantees.
- **Daemon plumbing:** bounded outbox and ingress, explicit `limits.rs`, and the SQLite catalogue.
- **Stop/reopen:** `manager/stop.rs` (241 lines) and `stop_worker.rs` (62) are tight for the cases they cover; owned process families likewise.
- **Source and output cursors:** needed because clients render at their own width.

## 3. Biggest questionable areas, ranked by payoff

1. **Panels use a paged, chunked, revision-hashed, per-reader-capability protocol where a capped snapshot would do.**
   - A task label goes through a 2 KiB preview, then 64 KiB offset reads (`session/task_views.rs:103-163`), with reply validators in `carlos-client/src/tasks.rs:108-200`.
   - The terminal then keeps cursor arrays and a "relative movement" resolver to keep a selection across 32-row pages (`remote/tasks_pages.rs:73-226`).
   - Task windows add server-side captured windows (8 slots, 8 MiB) guarded by a per-socket `Arc<AtomicBool>` (`manager/task_windows.rs:14-64`, `session/task_windows.rs:37-92`). That guard only fences off the same user's other devices.
   - Task lists and queues are human-scale. The existing 4,352-row ceiling at 1 KiB labels fits one 8 MiB frame.
   - Simplification: one capped page per panel revision with a `truncated` flag, and a single shared `text/read {handle, offset}` for the rare long field. Boundedness is preserved.
   - Scale: roughly 60 production files across five crates serve tasks and queue; the 13 I read total about 2.9k lines. Low thousands recoverable; I would not defend a tighter number.

2. **Panel read drivers live in the terminal, duplicated per family, on top of a second request-id correlation.**
   - `remote/queue_reads.rs:135-250` and `remote/tasks_reads.rs:129-315` are the same state machine: request, future, failure, revision reset, wait until `replica.sequence() >= page.seq`.
   - `queue_pages.rs` and `tasks_pages.rs` are the same pager; `tasks_overview.rs:86-267` is a third variant.
   - The client already maps id to `Kind` and validates (`pending.rs:179-341`, `client_replies.rs:61-120`). It then hands back `ReadResult{id}`, and the terminal matches the id again, with a 128-entry `retired` deque (`tasks_reads.rs:33-57`) and take-and-push-back routing (`queue_reads.rs:76-107`, `events.rs:55-60`).
   - Simplification: one generic pager in `carlos-client` that owns outstanding reads; UIs read settled rows. This removes the duplicates now.

3. **`Command`, `EngineCommand`, `EngineReply` and `ReadReply` are hand-copied mirrors.**
   - `manager/mutation_work.rs` is 361 lines, about 90% field-for-field rewrap plus five size checks.
   - The same pattern repeats in `manager/tasks.rs:131-259`, `read_commands.rs:34-164`, and 21 identical `self.reply` arms in `reads.rs:82-108`.
   - Limit defaults and clamps are restated in three crates (`pending.rs:270`, `manager/tasks.rs:182`, `task_views.rs:48`).
   - Core already depends on proto types throughout, so make each variant payload a named proto struct that `EngineCommand` carries, move size checks to one proto `validate()`, and let the owner reply with one serialisable value.
   - Scale: about 500-700 lines, and a new command drops from roughly eight edit sites to three.

4. **Bounded body conduit feeding an unbounded store (terminal only; lower confidence).**
   - Client windows and pause/resume backpressure (`bodies.rs`, `remote/source_formatter.rs:65-78`, `transcript_formatter.rs:267-313`) bound transit.
   - The formatter worker then keeps the whole original (`transcript_ansi_pages.rs:54-55`), which matches the recorded 238 MiB retention.
   - That file's own doc comment still says "Prototype" (lines 1 and 37) while being production.
   - The owner explicitly rejected a body cap, so I list this as a cost, not a cut.

5. **Fragmentation driven by the caps.**
   - `Device` is one 18-field object (`remote.rs:93-112`) whose `impl` is spread over about 70 files.
   - Several files sit just under 400 lines (`command.rs` 399, `transcript_formatter.rs` 393).
   - EP020's decision log moves `handle.rs` out "to preserve file size caps" (plan line 96).
   - This costs navigation and per-file import and doc overhead more than logic. Do not fix it on its own; items 1-3 dissolve much of it.

## 4. Sequencing

- **Before the WebSocket listener and Swift consumer make v1 sticky:** do item 3, ahead of EP020 milestone 3's device and operator commands. Decide item 1's wire shape, or mark the task-window, task-activity and overview families terminal-only. Move item 2's pager into `carlos-client`, or commit that the phone will not consume those panels.
- **Safe to defer:** item 4, item 5, and the 8 ms poll.
- **Do not rewrite:** owner threads and mailboxes, the retry store, outbox bounds, `Replica`, source cursors, stop/reopen, and process-family teardown.

## 5. Will each new UI multiply code?

- **Transcript, approvals, send, interrupt, operations: no.** `Replica`, `Operations` and `Bodies` are genuinely shared.
- **Panels: yes, as written.** The driver logic in item 2 sits in `src/app/remote`, not in `carlos-client`, which contradicts the "one copy" rule in `docs/engine-architecture.md`. A Swift client wanting queue or tasks would reimplement about 1.3k lines of subtle revision and sequence handling per panel pair.
- Fixing items 1 and 2 before EP022 is what keeps the per-UI cost down to layout only.
