# EP019b interim whole-roadmap review

Fable 5.2 (`claude-fable-5-2`), medium effort, read-only, 2026-11-14.
Reviewer session: `3beef6b6-7bac-40b2-a2b4-ca58e96196f5`. This is an interim review of unfinished work,
not EP approval. The reviewer read the plans and representative code and ran no tests.

+Review is done; I read the plans, docs and representative code only, and ran no tests or builds.

## Overall judgment

The architecture is sound: one owner per session, serde-only proto, a pure client, and writer-dispatch fences all fit EP020–022. The cost risk is ordering and breadth. EP019b keeps perfecting server-side families while its riskiest piece, the terminal consuming `carlos-client`, has not started. Machinery is being sized by imagined consumers.

## Ranked findings

1. **The render contract is unsettled while the cache for it is already built.**
   - The terminal lays out a whole `Message` per block (`src/app/transcript_render.rs`, `BlockPlan`/`DisplayMessage::full`).
   - `crates/carlos-client/src/bodies.rs` and `body_pieces.rs` keep 32 suffix windows of 256 KiB each, with seeks and discarded prefixes.
   - The plan's own Decision Log admits this "does not yet settle terminal Markdown/ANSI layout across arbitrary ranges". Fences, tables, diffs, ANSI state and exact copy do not window cleanly.
   - `rows.rs` already allows 32 MiB of inline rows, so the 256 KiB cap is inconsistent with it.
   - **Recommendation:** decide now that the terminal hydrates a complete contiguous body up to a stated cap, with windowing only beyond it. Otherwise the renderer gets rewritten or the window code gets discarded.

2. **Families are landing without a consumer.**
   - There are still about 446 `.session` reaches across 74 `src/app` files, and `carlos-client` is used only by `crates/carlosd/tests/client_wire.rs`.
   - The EP018a review advised against adding a family until a terminal workflow drives it through the dispatcher; goal, queue and task routes have daemon tests only.
   - **Recommendation:** stop widening. Cut over transcript, send, approve, interrupt and source reads next, then port each panel together with its family. Milestone 4 already allows the old path to remain during parity work.

3. **Task stop has stacked guards and three registries.**
   - One user action passes `TaskRevision`, `TaskRun`, `TaskReadId`, a `StopClaim` atomic with five retire/reconcile entry points (`session/task_stop_claims.rs`), an engine `Target` plus 15-second deadline (`engine/task_stops.rs`), `DispatchGuard`, and the helper tombstone table.
   - Pending state lives in three places: `session.task_stops`, `Owner.task_stops` and `claude.tasks.stops`.
   - The race is real: a replaced run must not be stopped. But a Codex turn interrupt and the Claude helper fence both name the run natively. Only Codex process termination lacks a native identity.
   - **Recommendation:** keep one prepare-time check plus the native-boundary check, keep reconciliation only for process PID/item evidence, and merge to one registry before committing.

4. **Four near-identical chunked-text protocols.**
   - `SourceChunk`, `OutputChunk`, `QueueText` and `TaskText` each carry their own revision, offset and completeness rules.
   - `TaskField` in `crates/carlos-proto/src/tasks.rs` has 24 variants, each chunk-addressable, including `OsPid` and `CpuPercent`.
   - Each one will multiply again in the client, UniFFI and Swift.
   - **Recommendation:** use one bounded text-read primitive addressed by entry, queue item or task field. Make `TaskDetail` a typed struct with short scalars inline, and allow long reads only for the few fields that are actually long.

5. **Tasks are half pull and half push.**
   - `TaskPage` carries `seq`, `revision`, `known`, `reading` and `closed`, while "aggregate interests" and "numbered task facts" are both still open.
   - Polling with revisions is a poor fit for a phone on WebSocket. Per-client interest aggregation is a second subscription system.
   - **Recommendation:** publish a compact task list as numbered shared facts and keep detail and long text as one-shot reads. Skip interest aggregation unless a native read proves costly.

6. **Process weight (minor).**
   - The 400-line rule has split the client into about 25 files (nine `client_*.rs`).
   - The living plan is mostly per-checkpoint log.
   - The uncommitted stop work touches 37 files.
   - **Recommendation:** none of this is wrong, but commit smaller and compress the plan log.

## Settle before more implementation

- **Body contract:** whole-body hydration versus windowed rendering for the terminal (finding 1).
- **Task model:** pushed facts versus polled reads with interests (finding 5).
- **Ordering:** cutover-first for the remaining families (finding 2).
- **Text reads:** one primitive or four (finding 4).

Finishing the stop slice in simplified form is fine. I would not start older task windows, history or interests until the first two are decided.

## Codex assessment and next work

I accept the ordering finding: finish the stop fixes and then prove a terminal transcript
consumer before widening the remaining server APIs. The terminal must preserve exact source,
Markdown, ANSI colors, wraps and copy. A complete-body cache within an explicit aggregate
budget is a prototype to measure, not yet an accepted replacement for the current fidelity
requirements. The current source windows do not prove this behavior.

I accept the guard simplification for agents. A Codex interrupt already names the exact native
turn, and Claude checks the native run inside the helper. I removed Rust's child-turn and
Claude-run reconciliation hooks. Carlos still checks the requested run before preparing work,
retires queued writes after root/connection changes and rechecks process PID/item evidence,
because native process termination accepts only processId. TaskRevision protects read pages;
TaskReadId protects an outstanding read. Neither is another required field in task/stop.

The engineering reviewer separately found that Claude's pipe writer can wait 500 ms and that
its failure may follow a write. Stops now use the existing control workers. Preparation errors
remain definite rejections; unconfirmed native writes remain uncertain. An actual-daemon test
fills the owned compiled helper's pipe and checks readable sockets before its writer timeout.

The core claim map currently owns write permissions, the engine map owns private outcomes,
and Claude's old map preserves the embedded terminal's once-per-run behavior. These are
different responsibilities, but the old Claude stop map must be reconsidered when that panel
moves to the client. Do not add another public stop-state cache.

I accept compact numbered task facts as the direction for the client. Add native read
interests only when the terminal workflow needs them, particularly a live child activity read.
Do not build older windows/history/interest infrastructure before the transcript prototype.

Keep the typed entry, queue and task addresses rather than adding a generic public native
object router. Short task scalar properties already arrive complete in task/detail; clients
need not chunk-read PIDs. Share internal UTF-8 slicing semantics where it removes repeated
code, then judge the long-field API through an actual panel consumer. This decision preserves
the fixed typed request surface without claiming the current field model is final for Swift.

Keep completed evidence in focused reports and keep the active plan's next steps clear.
The final EP review must cover the actual terminal cutover and deletion, which do not exist yet.
