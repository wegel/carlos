# EP019b: run the terminal through the daemon and delete the embedded path


This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. The owner explicitly accepted EP019a in this conversation on 2026-11-14 and then requested autonomous execution. Work directly without implementation subagents. Do not edit archived plans.

## Purpose / Big Picture


After this step, closing a terminal leaves its conversation running in `carlosd`, the separate process that owns native Codex, Claude and Grok sessions. Another terminal can attach to the same conversation. Each terminal keeps its own editor, scroll position, selected task and expanded tool output. Existing terminal commands, colors and exact copy behavior must work through typed socket requests. The terminal must no longer own a backend or mutable core session.

The daemon already proves basic chat, approvals, running output, independent clients, private command recovery and saved-session restore. This step completes the terminal-facing commands, adds a pure shared Rust client, and removes the temporary embedded route. It does not add TCP, device pairing or phone code. Stop at owner review before EP020. The existing spike rule still forbids installing a spike executable before owner review, merging into main or pushing without the owner's request.

## Progress


- [x] (2026-11-14) Read repository rules, the terminal workflow inventory, current daemon interfaces and prior acceptance evidence. Confirm clean `iphone-spike` and main `a29b1a1`.
- [x] (2026-11-14) Record owner acceptance of EP019a and write this executable plan.
- [ ] Preserve EP019a release identities and rerunnable latency/memory evidence; run the initial workspace tests.
- [ ] Add bounded original-source reads and watches, explicit format-bound cursors, and bounded handling of large plain entries/pages. Prove split controls, Unicode, body replacement, exact source and independent clients through the daemon.
- [ ] Add typed native facts/commands for names, prompt edits, task reads/stops, queues, goals, settings/context/quota, summaries, host defaults and lifecycle. Preserve existing tokens and target/generation guards; prove two clients can choose different read targets.
- [ ] Add pure `carlos-client` for greeting, private retry rights, numbered facts, history, source/output cursors and operation-owned local echo. Test gaps, replacement, reconnect, malformed input and private recovery without sockets or clocks inside the crate.
- [ ] Route terminal startup and every existing workflow through the actual daemon and shared client. Preserve drafts, dictation, color/copy behavior, panels and startup choices. Add screen/request tests for any broken workflow.
- [ ] After socket parity passes, make the separate buildable commit `tui: remove embedded session and backend path`; remove direct session/backend ownership, temporary adapters and obsolete report consumers. Review that deletion explicitly.
- [ ] Pass final default/no-default tests, strict lint, release/native/terminal/process/dependency checks and matched socket speed/memory comparisons. Review directly, report exact identities/limits, archive the completed plan unchanged and stop for owner review.

## Surprises & Discoveries


The terminal still embeds `SessionState` in `src/app/state.rs`, consumes borrowed change records in `src/app/core_events.rs`, and reads native panel state directly. Replacing only the backend trait with socket forwarding would leave a second privileged session and cannot meet the deletion gate. The terminal must render a client-owned fact copy instead.

EP019a's normalized tool chunks deliberately strip terminal controls. `crates/carlos-core/src/session/output.rs` reads that normalized view from the single original body. The terminal needs the original bytes for its existing safe color renderer and exact copy, so it must request a separate source format with a cursor that cannot be reused for normalized output. The old eight-MiB snapshot preflight rejects a whole page or large plain row; the new path must bound text independently from metadata rather than raise frame budgets.

Main has not advanced beyond `a29b1a1` at plan start. EP019a's final source is `58e9901`, closeout `614f296`. Its terminal binary is 20,346,176 bytes, SHA-256 `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`. EP019a's raw artifacts remain under `/tmp/carlos-ep019a/`; its tracked report contains the exact methods if temporary evidence disappears.

## Decision Log


Decision: begin EP019b after the owner's explicit “OK. Approved. Ack.” Rationale: the requested EP019a review gate is satisfied; no further routine confirmation is needed. Date/Author: 2026-11-14, owner/Codex.

Decision: keep source/output transfer bounded separately from entry metadata, with format-bound cursors over one original stored body. Rationale: raising JSON frame budgets or reconstructing raw text from stripped pieces would lose copy/color fidelity or increase memory without a bound. Date/Author: 2026-11-14, Codex.

Decision: preserve the existing native asynchronous token/deadline mechanisms and aggregate explicit read interests per connection. Rationale: one client's selected task must not choose which reply another client accepts, and a second general job scheduler would duplicate native ownership. Date/Author: 2026-11-14, Codex.

Decision: implement the shared client without sockets, threads or clocks. Rationale: the terminal supplies bytes/time while future phones can reuse the same checked state changes with their own transport. Do not add bindings or schema generators before that consumer exists. Date/Author: 2026-11-14, Codex.

## Outcomes & Retrospective


The owner accepted the working headless daemon; EP019b is now active. No terminal cutover or new runtime behavior is claimed yet. The next action is preserving the accepted binaries and proving bounded original-source transfer before changing terminal ownership. The final result must include a separate deletion commit, not a permanent switch between two terminal modes.

## Context and Orientation


Work from `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`. Prefix shell commands with `rtk`. Recheck main and rebase if it moves, leaving the main checkout untouched. Keep Rust files within 400 lines and functions within 60; follow `.agents/CODE_STYLE.md`. Explicitly add intended files; never use `git -A`.

`carlos-proto` contains serde-only typed wire requests, responses and numbered events. `carlos-core` owns full source, native adapters and session logic. `carlos-core::engine` owns one thread per session and accepts typed commands through bounded lanes; its worker handles capture a connection and conversation and recheck permission at writer dispatch. `carlosd` owns local socket workers, per-client retry rights, the saved-session catalogue and the manager. The terminal currently owns an embedded session in `src/app/state.rs`; `ViewState` owns its local editor/render caches. The new `crates/carlos-client` will own the terminal's remote facts, never a core session.

A sequence is a per-session monotonically increasing event number encoded as a decimal string. A snapshot supplies the newest page at one such number. History pages contain older stable entry ids. A source/output cursor identifies a byte position and body revision for one entry and format; replacing the body invalidates it. A client operation identifies one submitted command with its authenticated client id and decimal counter. Shared facts reach all subscribers; private outcomes and recovered text belong only to the submitting client.

`docs/terminal-client-migration.md` is the exhaustive workflow inventory and `docs/core-report-routing.md` classifies existing report fields. Read both before moving a command family. `tests/ui/` preserves 183 terminal workflows, `tests/native_monitor/` preserves native side behavior, and `tests/daemon/` drives the actual daemon through deterministic native peers. These fixtures need no model, account or unrelated saved session. The current default suite passes 816 application tests, 296 core, nine protocol, 20 daemon and three compiler checks; some older tests are intentionally ignored.

## Plan of Work


### Milestone 1: preserve exact source through bounded reads


Add source format, cursor and chunk types under `crates/carlos-proto/src/`, with explicit source read/watch requests and reset behavior. Implement UTF-8 boundary-safe bounded reads in the core from the existing original body, including non-tool rows; do not clone the whole text to serve a chunk. Cursor validation must check entry, revision and format. Preserve split terminal escapes as original source, letting the terminal's existing renderer apply its safety rules. Keep normalized phone output unchanged.

Make snapshots/history/events carry bounded entry text with an explicit source descriptor when a body does not fit. A descriptor records enough revision/length information to fetch the remainder without mistaking it for truncated complete text. Large pages must shrink to a valid page and retain correct older cursors. Large live appends must use bounded chunks or an explicit source-changed fact so no enormous append is encoded. Reads and watches share existing per-peer count/byte limits and use generation checks; a lost source watch gets an explicit reset, never silent cursor advancement. Add real socket tests and core cursor tests with split escapes, Unicode, running append, replacement, long plain rows and large pages. No terminal cutover is accepted before this proof.

### Milestone 2: expose the remaining typed terminal workflows


Extend typed protocol modules by concern rather than enlarging a single command file. Add `session/rename`, `session/rewind` with a stable prompt entry target, task list/detail/history/stop requests with opaque run-bound addresses, native queue read/mutations with expected revisions, goal reads/mutations, runtime settings/capabilities, detailed context/quota/compaction, summary settings and host defaults. Add native saved-session discovery/resume and explicit lifecycle controls needed by current CLI startup. Never accept arbitrary backend method names or raw native parameters from clients.

For every family, move its public facts into protocol data, retain validation and native translation in core, and route mutation results through the existing private operation store. Prepare I/O outside session borrows and check conversation/generation when completing. Keep pending/unknown/error states explicit. Aggregate clients' read interests by stable target; native callbacks may update shared facts but cannot select a client's editor. Keep only accepted host defaults on disk. Preserve Ctrl+M's context-size editor, attached Codex read-only choice and Claude shell override. Complete the report-field map as each family moves and remove editor/focus instructions from shared notices. Test each promoted family through the actual dispatcher, including stale targets and two independent client choices.

### Milestone 3: share checked client state without transport


Add the workspace/default member `crates/carlos-client`, depending on proto and serde/serde_json only unless a measured, reviewed need arises. Split codec, connection/retry state, transcript/history, output/source caches and operation tracking by concern. Expose a client object that consumes one decoded envelope or input command and returns changed entry/session ids and outgoing typed requests. The platform performs socket I/O and supplies time for reconnect backoff.

Reject sequence gaps and request a snapshot instead of applying later deltas to stale facts. Apply snapshots atomically, merge older history without duplicating ids, discard replaced source caches and keep exact ClientOperation ownership for local echo. New daemon instances invalidate replay/retry assumptions; do not automatically resend possibly written work. Bound outstanding requests, private records and cached output. Test the pure state changes with fixed times and envelopes, then use the library against a real temporary daemon to prove codec/dispatcher agreement. Extend the dependency checker to forbid core/native/UI/audio dependencies under the shared client.

### Milestone 4: drive the terminal through its first remote client


Add a small Unix socket transport and daemon discovery/launch code in terminal platform modules. Preserve current CLI backend/cwd/socket/resume/Ralph/settings choices through typed daemon requests. Use a private runtime socket/state directory by default; starting two terminals must converge on one daemon without unlinking a live socket. Closing a terminal detaches; explicit session stop closes backend work. Keep the existing deterministic native peers and private test homes; ensure daemon cleanup in the test harness targets only processes it started.

Replace `AppState.session` with the shared client's session facts. Adapt render inputs and local panel editors to typed facts, hydrate source lazily and watch only needed output. Keep typing/scrolling independent of sockets. History navigation uses stable prompt entry ids; private failures restore only the originating command's text, never equal-text matching. Dictation continues to produce ordinary text locally. Route all terminal input and panel commands through the typed facade, without a hidden direct-core fallback. Retain the old path only while proving parity during this milestone.

Run every existing terminal workflow through the actual daemon. Assert visible screen and native requests, preserve exact copy/color behavior and add two-client task/editor isolation tests. Fix regressions with deterministic fixtures rather than weaken assertions. Check startup cards, early typing, native restore, malformed requests, queue/goal/settings uncertainty, approval races, long output, history replacement and disconnect. Record parity evidence before deleting the old path.

### Milestone 5: delete the embedded path, measure and close


Make a separate buildable commit named `tui: remove embedded session and backend path`. Delete terminal backend/session ownership, direct native launch/control calls, obsolete report enums/bridge consumers and temporary execute_embedded adapters. Keep only remote platform launch/discovery and pure rendering helpers. Review the deletion diff for hidden privileged paths. Production terminal dependencies must not pull native engine code after the move; test utilities may exercise core independently without restoring a production shortcut.

Run final feature/lint/release/dependency/native/terminal gates and compare matching release binaries with preserved EP019a and the original architecture reference. Exercise socket replay with the same synthetic/captured fixture; separate core apply, export/encoding, socket/client and terminal render costs. Measure resident memory rather than infer heap from wire budgets. Preserve raw samples, exact source/compiler/binary identities and honest workload limitations. Use the engineering reviewer prompt directly, label self-review honestly per the owner's no-subagent request, and resolve blocking findings. Write final reports, archive this completed plan unchanged, update PROGRAM_PLAN and stop at owner review before EP020.

## Concrete Steps


All commands run from `/var/home/wegel/work/perso/carlos-daemon`. Preserve accepted `/tmp/carlos-ep019a/final-carlos` and `final-carlosd` into `/tmp/carlos-ep019b/` with hashes before rebuilding. If they are missing, build source `58e9901` in an isolated worktree/target and record the new compiler/hash. Keep the installed `~/.local/bin/carlos` unchanged until owner review.

    rtk proxy cargo test --workspace
    rtk proxy cargo test --workspace --no-default-features
    rtk proxy cargo clippy --workspace --all-targets -- -D warnings
    rtk proxy cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy bash scripts/test.sh --codex-contract
    rtk proxy bash scripts/test.sh --release --codex-contract
    rtk proxy python3 scripts/check-core-dependencies.py
    rtk proxy cargo run --release -p carlosd --example perf_replay -- 1000

Use targeted core/protocol/daemon tests while developing, followed by `cargo test` after meaningful changes. The release runner must test the exact executable whose identity appears in the final report. Preserve final logs under `/tmp/carlos-ep019b/` and screen/native traces under `target/ui-artifacts/` and `target/daemon-artifacts/`. Update these steps with new stable commands as the first client fixture works.

## Validation and Acceptance


Two real terminals must attach to one temporary daemon session, see the same numbered facts and retain different drafts, selected tasks and expanded output. Closing either terminal must leave native work alive. The existing deterministic screen/request workflows must all pass through socket dispatch, including native backend choices, approvals/questions/browser consent, queue/goal/task panels, settings/context/quota, names, prompt editing/fork recovery, Ralph, transcript/copy and dictation feature modes.

Large plain rows/pages and running colored output must transfer in bounded chunks without truncating original source, duplicating it in core or exceeding queue budgets. Wrong-format/foreign/retired cursors fail explicitly. Gaps trigger fresh snapshots, and private retries/results survive live reconnect without submitting twice. A daemon restart never automatically resends uncertain work or starts saved Ralph. Stale native completions cannot mutate replacement sessions. One client's panel target/editor cannot change another client's accepted reply.

The final production terminal owns no core SessionState, BackendClient or native protocol route. The separate deletion commit and dependency checks must prove that boundary as well as tests. No generic native JSON passthrough or permanent embedded mode may remain. Matched performance checks must satisfy the architecture's speed rule and explain memory differences with actual measurements. All checks pass before marking this plan complete.

## Idempotence and Recovery


Tests use private temporary state/native homes and only stop captured owned processes. Reject unsafe socket paths and refuse to unlink a live daemon. Preserve original source and native history; failed reads or changed cursors request explicit refresh without altering backend facts. Treat possibly written mutations as uncertain and recover by operation identity, never automatic replay. Keep small buildable commits throughout, including the explicit deletion commit; do not edit archived plans or install experimental runtime changes before owner review.

## Artifacts and Notes


Accepted EP019a evidence is `.agents/reports/exec019a-final-report.md`, its direct review is `.agents/reports/exec019a-engineering-review.md`, and its archived plan is `.agents/done/EXECPLAN_019a_headless_daemon.md`. Its default terminal/daemon source is `58e990100a708d322e6a113b683e42918c243846`; closeout is `614f296`. Its final release runner artifacts are `target/ui-artifacts/run-hzGFMz`. The original pre-EP016 reference and EP018a preserved reference are identified in that report. Do not invent fresh comparisons if temporary binaries are missing.

## Interfaces and Dependencies


Keep public wire types in serde-only `carlos-proto`. Add typed source chunk/cursor/format and native command/fact modules there. Extend `EngineCommand`/`EngineReply` in core with those typed calls, preserving bounded ordinary/control/read lanes and captured work permissions. The daemon retains client interests, retry records and socket deadlines; no core owner may wait for a device or disk. `carlos-client` owns checked client facts and codec only; platform code owns sockets, clocks, terminal and audio. Types may evolve while version 1 remains provisional, but every promoted field must have a concrete consumer/test and the current protocol docs must match it.

## Engineering Review


Plan author self-check: the source, native command families, report-field map and terminal inventory were read before writing this plan. EP019a owner acceptance is explicit; the next gate is after working terminal parity and deletion, not after writing this plan. No runtime proof or independent review is claimed yet. Use `.agents/reviewers/engineering_reviewer.md` for the final direct review and copy findings, guidance and verdict here.

Update note (2026-11-14): created EP019b after owner acceptance of EP019a. Require bounded raw source and large-row support, complete typed terminal controls, a pure shared client, actual socket workflow parity and a separate deletion commit before owner review. Preserve the existing no-install/no-merge/no-push spike rules.
