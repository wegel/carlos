# Review of Carlos at `iphone-spike` 51c4a00, after EP018a

I read docs, reports and source only; test and performance figures are the report's claims, not mine.

## 1. Verdict: sound with course corrections

EP018a fixed what it claimed in three areas, and I verified each in source:
- **Running output:** bounded, cursor-resumed reads with no second stored body (`session/output.rs:41-81`, `output_cursor.rs:51-65`).
- **Client inputs:** `BackendContext` now holds only a panel scope (`backend_update.rs:12-14`); composer text, confirmation tokens and focus are gone from core handlers.
- **Guards:** the lexer is deleted and the remaining check is 86 lines over `cargo metadata`.

Two prior concerns are only partly solved: backend waits (three commands converted) and terminal wire coverage (documented, not built). Both are correctly assigned to EP019, but EP019 as now scoped is too large.

## 2. Ranked concerns

**1. EP019 is three plans in one. (BEFORE EP019, planning only.)**
- `docs/terminal-client-migration.md:49-53` puts the daemon, about ten typed command families, `carlos-client`, parity for all terminal workflows and deletion of the embedded path into one plan.
- The terminal reaches into the session at about 452 non-test sites in 74 `src/app` files, up from the roughly 377 the prior review counted.
- Wire output strips terminal controls (`session/output_normalize.rs:62-72`), while the terminal renders and copies the original body. Parity needs a raw-body decision that nobody has made.
- Likely consequence: a months-long embedded and daemon double path, or parity pressure producing a generic escape hatch.
- Correction: split into EP019a (owner loop, real adapters, Unix socket, headless client on the current commands) and EP019b (families, shared client, terminal cutover, deletion), with owner review between.

**2. Real adapters cannot yet execute concurrently. (DURING EP019, first gate.)**
- `BackendWork::execute` takes `&dyn BackendClient` (`command_work.rs:86`).
- `ClaudeClient` holds `handler: RefCell<...>` and its receiver (`claude_backend/mod.rs:113-119`), so it cannot be shared across threads.
- The delayed-worker test builds a fake `Peer` on the worker thread (`command_work_tests.rs:16-87`). It proves the session stays readable, not that a real adapter can be called from a worker.
- Only send, fork and rename were converted. Interrupt still blocks up to 10 s inside `&mut SessionState` (`turn_interrupt.rs:23`), and about 35 other `client.` calls remain across 20 session files.
- Likely consequence: a stalled owner loop, or a mutex around the client that queues an interrupt behind a 10 s send.
- Correction: split the trait into a shareable command handle and an owner-held receiver/normalizer. Convert interrupt and approval replies before any socket command is accepted.

**3. Facts still leave the core through return values, outside the numbered stream. (DURING EP019; state the rule in the plan.)**
- `BackendUpdate`, `ClaudeUpdate`, `TurnUpdate`, `CommonUpdate` and `CodexClosed` carry outcomes, hints and retirements.
- `SubmissionOutcome` exists in `carlos-proto/src/submissions.rs`, but `Notification` has no variant for it (`events.rs:68-75`).
- `ClaudeEffect` still holds view instructions: `CodexUnfocus`, `SwitchControls`, `ResetControls` (`claude/receive_update.rs:36-38`), applied at `src/app/claude_updates.rs:79-84`.
- Likely consequence: the daemon hand-translates each report, and a recovery outcome is lost if its client is disconnected.
- Correction: classify every report field as a numbered session fact, a private per-operation notification, or deleted. The report enums should not survive the cutover.

**4. The one-slot `Busy` rule is a single-client policy. (DURING EP019.)**
- `command_guard.rs:30-36` and `prompt_commands.rs:64-70` reject a second mutation while one waits.
- With two clients, a phone send arriving during a terminal send is refused with "draft kept" for up to 10 to 20 s.
- Correction: the daemon queues commands per session in arrival order and keeps `Busy` internal. Interrupt and approval answers bypass that queue.

**5. Small leftovers that contradict the stated boundary. (DURING EP019, as families land.)**
- Terminal key names remain in shared notices, contrary to the report's claim: `prompt_commands.rs:172-175`, `codex/lifecycle.rs:63,70`, `codex/goals.rs:94`, `codex/settings_reply.rs:66`, `codex/frame_install.rs:62`. A phone would read "F5 reconnects".
- There are two correlation ids: `client_message_id: String` (`commands.rs:71`) and `ClientOperation` (`submissions.rs:9`). Keep one, and take the client id from the connection.
- `Entry.client_message_id` is still always null (`event_rows.rs:27`).
- `TurnUpdate.submitted` and `name_saved` clear the composer by text match (`src/app/turn_submit.rs:114-125`).
- `backend_context()` fabricates `visible: true` (`backend_update.rs:45-55`); `PanelScope` should shrink to connection identity.

The wide all-backend `SessionState` (`state.rs:37-66`) is unchanged and remains a LATER item. It gained one field and is not getting worse.

## 3. Complexity

Keep:
- The opaque output cursor with bounded parser state and sparse revisions. It is about 300 lines and removes the quadratic refetch.
- Prepare, execute and finish with `WorkGuard` (`command_guard.rs:68-86`). The stale-completion fencing is the hard part and is done.
- `ClientOperation` ownership checks in the terminal (`claude_updates.rs:90-138`).
- Stable ids, the bounded journal, full-object replacement, thread per session, serde-only proto.
- The compile-fail doctests and the Cargo dependency check.

Simplify or avoid:
- Do not build a watch scheduler, replay ring and connection limits beyond what the headless client needs in EP019a.
- Do not add a command family until a terminal workflow test drives it through the dispatcher.
- Do not give the terminal a second local transcript model plus the borrowed one; pick one at cutover.
- Do not extend the prepared-work pattern to reads that already use request tokens and deadlines.
- Keep bindings, schemas and the delta merger deferred, as the docs now say.
- Give the embedded path a deletion commit named in the EP019b plan, not "after parity".

## 4. Proceed to EP019?

Yes. No code change is a prerequisite; the core boundary is clean enough for socket work.

The one real prerequisite is in the plan text:
- Split EP019 as in concern 1.
- Make its first milestone the adapter test from concern 2, on real Codex and Claude adapters with deterministic peers. It should include a reset before dispatch and an interrupt during a waiting send.
- Write the report-classification rule from concern 3 into the plan.

If the first milestone fails or needs a mutex around the whole client, stop and review before building the listener.
