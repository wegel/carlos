# EP019b: run the terminal through the daemon and delete the embedded path


This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. The owner explicitly accepted EP019a in this conversation on 2026-11-14 and then requested autonomous execution. Work directly without implementation subagents. The owner separately authorized a Fable 5.2 medium review before each auto-approval; this reviewer may read but must not implement changes. Do not edit archived plans.

## Purpose / Big Picture


After this step, closing a terminal leaves its conversation running in `carlosd`, the separate process that owns native Codex, Claude and Grok sessions. Another terminal can attach to the same conversation. Each terminal keeps its own editor, scroll position, selected task and expanded tool output. Existing terminal commands, colors and exact copy behavior must work through typed socket requests. The terminal must no longer own a backend or mutable core session.

The daemon already proves basic chat, approvals, running output, independent clients, private command recovery and saved-session restore. This step completes the terminal-facing commands, adds a pure shared Rust client, and removes the temporary embedded route. It does not add TCP, device pairing or phone code. The owner delegated approval of completed EPs on 2026-11-14 and requested continued work through the next EPs. Pass every check and the direct engineering review; obtain the owner-requested Fable 5.2 medium whole-roadmap review and address its findings using judgment. Record the auto-approval, then begin EP020. Keep the installed executable unchanged; do not merge into main or push without the owner's request.

## Progress


- [x] (2026-11-14) Read repository rules, the terminal workflow inventory, current daemon interfaces and prior acceptance evidence. Confirm clean `iphone-spike` and main `a29b1a1`.
- [x] (2026-11-14) Record owner acceptance of EP019a and write this executable plan.
- [x] (2026-11-14) Preserve EP019a release identities under `/tmp/carlos-ep019b/baseline-identities.json` with both executables; initial workspace tests pass. Prior repeatable measurements remain identified in EP019a's report.
- [x] (2026-11-14) Add exact bounded source reads, separate source cursors, independent format watches, large-plain source descriptors and byte-budgeted pages. Five new core tests, eighteen daemon workflows, 183 terminal workflows, 23 native-monitor workflows and installed Codex contracts pass; source commit `5e1d793`, artifacts `target/ui-artifacts/run-v0o1a7`.
- [x] (2026-11-14) Promote names and rewind/fork through the ordered lane, with checked follow-up work and private recovery. The edit gate passes 21 actual-daemon, 183 terminal and 23 native-monitor workflows, installed Codex contracts and helper/architecture checks at `target/ui-artifacts/run-yRfXLD`. Native fork lifetime and early-helper tests pass. Removing captured Codex fork permission makes its new native test fail; restored source is being rechecked before commit.
- [x] (2026-11-14) Commit the checked edit path as `7ac8446`; restored workspace tests and strict Clippy pass, including the final Codex guard test.
- [x] (2026-11-14) Replace the one selected-child detail cache with eight independently addressed native reads sharing one eight-MiB retained-data budget. Four new core tests prove out-of-order replies, isolated release/expiry and shared byte limits. Full workspace tests and strict Clippy pass; all 21 daemon, 183 terminal and 23 native-monitor workflows plus installed Codex contracts pass at `target/ui-artifacts/run-gK5mGB`.
- [x] (2026-11-14) Add direct revision-checked source seeking for a bounded client cache. Distinct lossless byte types, UTF-8-safe windows, two new core tests and updated wire fixtures pass workspace tests/strict lint. All 21 actual-daemon workflows pass at `target/daemon-artifacts/ep019b-seek`.
- [x] (2026-11-14) Complete the bounded source/page prerequisite above; seek commit is `36bfab5`. Terminal color/copy proof remains part of the socket cutover.
- [x] (2026-11-14) Fix approval event ordering before the shared client consumes it: publish anchors first and include order-only moves. The new test fails with the old publisher. Full gate passes 21 daemon, 183 terminal, 23 native-monitor workflows and installed Codex contracts at `target/ui-artifacts/run-VdDHRd`.
- [x] (2026-11-14) Preserve source roles, steered prompts, diff format/file paths and early failure flags in wire facts. Three core tests and one client test pass; the full gate passes 22 daemon, 183 terminal, 23 native-monitor workflows plus installed Codex checks at `target/ui-artifacts/run-K0tVof`.
- [ ] Add typed native facts/commands for names, prompt edits, task reads/stops, queues, goals, settings/context/quota, summaries, host defaults and lifecycle. Preserve existing tokens and target/generation guards; prove two clients can choose different read targets.
- [x] (2026-11-14) Add the pure shared client codec and bounded fact/history/card windows. Twelve client tests, workspace tests, strict Clippy and 15 architecture checks pass. The checker allows only reviewed data packages below the client.
- [x] (2026-11-14) Add private connection credentials/counters and operation-owned drafts/echoes. Twenty client tests, the actual daemon wire test, workspace tests and strict lint pass; 15 architecture checks pass. No client method automatically resends a mutation.
- [ ] Complete `carlos-client` for greeting, private retry rights, numbered facts, history, source/output cursors and operation-owned local echo. Test gaps, replacement, reconnect, malformed input and private recovery without sockets or clocks inside the crate.
- [ ] Route terminal startup and every existing workflow through the actual daemon and shared client. Preserve drafts, dictation, color/copy behavior, panels and startup choices. Add screen/request tests for any broken workflow.
- [ ] After socket parity passes, make the separate buildable commit `tui: remove embedded session and backend path`; remove direct session/backend ownership, temporary adapters and obsolete report consumers. Review that deletion explicitly.
- [ ] Pass final default/no-default tests, strict lint, release/native/terminal/process/dependency checks and matched socket speed/memory comparisons. Review directly, report exact identities/limits, archive the completed plan unchanged, record delegated auto-approval and continue to EP020.

## Surprises & Discoveries


The captured Codex async-control and callback-error paths checked the connection at enqueue but bypassed the captured permission at writer dispatch. The new control test passes with the fix and fails when that async path is deliberately restored (`goal-guard-negative.log`). Remote goal commands now retain their own request lifetime, so a fork/close retires a queued write as soon as core drops the request, and the owner returns an uncertain private result rather than leaving the old command pending. The goal family is still being checked through the actual daemon; do not mark it complete yet.


The terminal still embeds `SessionState` in `src/app/state.rs`, consumes borrowed change records in `src/app/core_events.rs`, and reads native panel state directly. Replacing only the backend trait with socket forwarding would leave a second privileged session and cannot meet the deletion gate. The terminal must render a client-owned fact copy instead.

An old Grok restore check waited for “History complete” inside a wire notification. With the new explicit large-body descriptor, that full text belongs in a source read. The check now waits for Ready and reads the restored source before asserting its contents. A new Codex fixture initially sent an item-completed record without its required final text, which correctly replaced the body with empty text; the fixture now uses the turn-completed boundary after its streamed deltas. No native completion behavior was weakened.

Promoting rewind exposed two native gaps. Captured Codex handles delegated a fork's validation/mutation calls to the unguarded connection; the fork now uses the captured facade throughout. Claude's fork outlives its initial request, so the command's short write permission cannot also represent the entire edit. The adapter confirms that first guarded write before releasing the ordinary slot, while a separate edit lifetime and originating operation protect later helper replacement and failure facts. The core registers the edited draft before dispatch so an early helper switch cannot lose it or restart it on a late write acknowledgement. Three actual-daemon edit workflows pass for names, Codex fork/send and compiled Claude fork/acknowledgement with original native history unchanged.

The old Codex panel retained only one selected child's activity. Its native tokens and deadlines now address a map of at most eight child reads, and all cached child items/tails share the existing eight-MiB budget rather than getting eight MiB each. Replies match original root/epoch/token/method/parameters and cannot consult another client's selected row. Releasing one target prunes only that request. The embedded terminal still clears its own reads on navigation, while the future daemon facade must aggregate explicit interests and release a target only when its last client leaves. The native task-list older-page window also remains global; it must be separated from client choices before exposing complete task paging. Do not mistake the detail-cache prerequisite for a complete public task family.

A bounded client cache also needs direct reads at earlier byte positions. Keeping only a forward cursor would force it to transfer a large prefix again when the user scrolls back. `entry/source/seek` now checks the expected body revision, rounds down to a UTF-8 boundary and returns at most 64 KiB directly from the original string. Source chunks report their exact starting offset without exposing cursor internals. ByteOffset and ByteLength remain distinct types and serialize losslessly as decimal strings. This is a concrete terminal consumer need; it does not add another stored body or parser.

EP019a's normalized tool chunks deliberately strip terminal controls. `crates/carlos-core/src/session/output.rs` reads that normalized view from the single original body. The terminal needs the original bytes for its existing safe color renderer and exact copy, so it must request a separate source format with a cursor that cannot be reused for normalized output. The old eight-MiB snapshot preflight rejects a whole page or large plain row; the new path must bound text independently from metadata rather than raise frame budgets.

Main has not advanced beyond `a29b1a1` at plan start. EP019a's final source is `58e9901`, closeout `614f296`. Its terminal binary is 20,346,176 bytes, SHA-256 `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`. EP019a's raw artifacts remain under `/tmp/carlos-ep019a/`; its tracked report contains the exact methods if temporary evidence disappears.

The shared client exposed a card-order defect: the publisher sent new cards before the new cards used as their anchors, and omitted order-only moves. The publisher now emits from tail to head and compares both card facts and the next-card address. The new deterministic test fails with the old publisher (`approval-order-negative.log`); the full terminal/native gate passes with the fix.

## Decision Log


Decision: accept the owner’s 2026-11-14 grant to auto-approve completed EPs and continue through subsequent EPs. Rationale: the owner is going to sleep and explicitly removed the need for a human pause between EPs. Keep all checks and the direct review; record each approval honestly and continue only after the EP is actually complete. This does not authorize merge/push/install actions. Date/Author: 2026-11-14, owner/Codex.


Decision: begin EP019b after the owner's explicit “OK. Approved. Ack.” Rationale: the requested EP019a review gate is satisfied; no further routine confirmation is needed. Date/Author: 2026-11-14, owner/Codex.

Decision: keep source/output transfer bounded separately from entry metadata, with format-bound cursors over one original stored body. Rationale: raising JSON frame budgets or reconstructing raw text from stripped pieces would lose copy/color fidelity or increase memory without a bound. Date/Author: 2026-11-14, Codex.

Decision: preserve the existing native asynchronous token/deadline mechanisms and aggregate explicit read interests per connection. Rationale: one client's selected task must not choose which reply another client accepts, and a second general job scheduler would duplicate native ownership. Date/Author: 2026-11-14, Codex.

Decision: implement the shared client without sockets, threads or clocks. Rationale: the terminal supplies bytes/time while future phones can reuse the same checked state changes with their own transport. Do not add bindings or schema generators before that consumer exists. Date/Author: 2026-11-14, Codex.

Decision: keep a Claude fork's initial write permission separate from its asynchronous edit lifetime, and include the originating operation in internal fork facts. Rationale: retiring an ordinary command after write acknowledgement must not cancel its own already dispatched fork, while a reset/new edit must prevent later helper replacement or failure from targeting the new conversation. Date/Author: 2026-11-14, Codex.

Decision: retain independent native child read tokens/deadlines under one combined cache budget. Rationale: two clients may choose different tasks, but each new view must not multiply the old per-session byte allowance. The current facade must still account for independent task-list pages and client interest lifetimes before public task commands are complete. Date/Author: 2026-11-14, Codex.

Decision: add a revision-checked byte seek to original source before building client cache behavior. Rationale: a bounded cache must revisit old windows without scanning or transferring their prefixes; opaque forward cursors alone cannot provide that. Keep byte offsets/lengths separate from sequence types. Date/Author: 2026-11-14, Codex.

Decision: send approval anchors before dependent cards and include order-only changes. Rationale: clients can apply each fact immediately without retaining a second pending-card graph or guessing where a missing anchor belongs. Date/Author: 2026-11-14, Codex.

Decision: keep only a bounded fence of changed unloaded row ids when merging history, rather than retaining a second event log. Rationale: a late page must not restore old text, while unrelated updates should not force the client to restart safe reads. Discarded fence entries raise the oldest permitted page cursor. Date/Author: 2026-11-14, Codex.

Decision: keep submitted prompt/edit/queued text under its exact client operation and require the original text in private recovery. Rationale: a response can arrive before or after its shared prompt row, and neither event can clear another draft or cause duplicate local echo. On lost daemon/client rights, pending work becomes uncertain rather than a new send. Date/Author: 2026-11-14, Codex.

Decision: preserve tool-call versus output roles, source format and file paths explicitly in Entry/EntryFacts. Rationale: the terminal already consumes those concrete source facts; reconstructing them from labels/text loses steering labels, code highlighting and diff behavior. Crossing the inline/diff boundary emits a row replacement even without a text edit. Date/Author: 2026-11-14, Codex.

## Outcomes & Retrospective


The owner accepted the working headless daemon; EP019b is now active. The core and daemon serve original source in UTF-8-safe chunks of at most 64 KiB, with separate entry/revision-bound cursors from normalized output. Both watch formats share the existing scheduler and combined count/byte budgets. Large plain rows carry explicit source descriptors instead of inline bulk text; escaped small rows reduce a page's row count before cloning. Five core tests and two daemon workflows prove exact source, split controls, format rejection, replacement and large-body/page recovery. Source commit `5e1d793` passes the complete existing gate.

Typed names and earlier-prompt edits now run through the actual dispatcher. Ordered fork/send chains keep one private retry identity, definite validation failures preserve only their owner's text, and asynchronous Claude forks retain checked edit lifetimes and exact acknowledgement. The full edit gate passes 21 daemon workflows, 183 terminal workflows, 23 native-monitor workflows, installed Codex contracts, 61 helper tests and 13 architecture checks. New core/native tests prove early helper adoption and retirement at writer dispatch and while awaiting a fork reply. The final additional Codex guard test passes and fails with the old unguarded code restored. Next are the remaining typed native panels/settings/lifecycle, the pure shared client, terminal source/render consumers and actual socket cutover. No terminal cutover is claimed. The final result must include a separate deletion commit, not a permanent switch between two terminal modes.

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

Run final feature/lint/release/dependency/native/terminal gates and compare matching release binaries with preserved EP019a and the original architecture reference. Exercise socket replay with the same synthetic/captured fixture; separate core apply, export/encoding, socket/client and terminal render costs. Measure resident memory rather than infer heap from wire budgets. Preserve raw samples, exact source/compiler/binary identities and honest workload limitations. Use the engineering reviewer prompt directly, label self-review honestly per the owner's no-subagent request, and resolve blocking findings. Write final reports, archive this completed plan unchanged, update PROGRAM_PLAN, record the delegated auto-approval and continue to EP020.

## Concrete Steps


All commands run from `/var/home/wegel/work/perso/carlos-daemon`. Preserve accepted `/tmp/carlos-ep019a/final-carlos` and `final-carlosd` into `/tmp/carlos-ep019b/` with hashes before rebuilding. If they are missing, build source `58e9901` in an isolated worktree/target and record the new compiler/hash. Keep the installed `~/.local/bin/carlos` unchanged while building the spike.

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


Tests use private temporary state/native homes and only stop captured owned processes. Reject unsafe socket paths and refuse to unlink a live daemon. Preserve original source and native history; failed reads or changed cursors request explicit refresh without altering backend facts. Treat possibly written mutations as uncertain and recover by operation identity, never automatic replay. Keep small buildable commits throughout, including the explicit deletion commit; do not edit archived plans or install experimental runtime changes without an explicit install request.

## Artifacts and Notes


Accepted EP019a evidence is `.agents/reports/exec019a-final-report.md`, its direct review is `.agents/reports/exec019a-engineering-review.md`, and its archived plan is `.agents/done/EXECPLAN_019a_headless_daemon.md`. Its default terminal/daemon source is `58e990100a708d322e6a113b683e42918c243846`; closeout is `614f296`. Its final release runner artifacts are `target/ui-artifacts/run-hzGFMz`. The original pre-EP016 reference and EP018a preserved reference are identified in that report. Do not invent fresh comparisons if temporary binaries are missing.

## Interfaces and Dependencies


Keep public wire types in serde-only `carlos-proto`. Add typed source chunk/cursor/format and native command/fact modules there. Extend `EngineCommand`/`EngineReply` in core with those typed calls, preserving bounded ordinary/control/read lanes and captured work permissions. The daemon retains client interests, retry records and socket deadlines; no core owner may wait for a device or disk. `carlos-client` owns checked client facts and codec only; platform code owns sockets, clocks, terminal and audio. Types may evolve while version 1 remains provisional, but every promoted field must have a concrete consumer/test and the current protocol docs must match it.

## Engineering Review


Plan author self-check: the source, native command families, report-field map and terminal inventory were read before writing this plan. EP019a owner acceptance is explicit; the EP can be auto-approved only after working terminal parity, deletion and all final checks, not after writing this plan. No runtime proof or independent review is claimed yet. Use `.agents/reviewers/engineering_reviewer.md` for the final direct review and copy findings, guidance and verdict here.

Update note (2026-11-14): created EP019b after owner acceptance of EP019a. Require bounded raw source and large-row support, complete typed terminal controls, a pure shared client, actual socket workflow parity and a separate deletion commit before owner review. Preserve the existing no-install/no-merge/no-push spike rules.

Update note (2026-11-14, source proof): preserved accepted executables and passed initial/full workspace tests and strict Clippy. Added exact-source core/protocol/daemon routes, independent format watches using one shared scheduler, byte-budgeted pages and explicit out-of-line plain source. Added five core boundary tests and two real daemon source tests. Existing restore assertions now read a descriptor-backed body; full terminal/native runner is being rerun. Remaining work includes terminal source consumer/color/copy proof, typed native controls, shared client, socket parity, deletion and final review/measurements.

Update note (2026-11-14, edit proof in progress): the source gate passes all eighteen daemon, 183 terminal and 23 native-monitor workflows, installed Codex contracts and helper checks. Added typed rename/rewind, ordered fork/send continuation, definite prepare failures versus uncertain native results, and operation-owned recovered text. Added real Codex and compiled Claude edit process proofs plus guard/lifetime/early-switch tests. Current edit source passes workspace tests and strict Clippy; the complete terminal/native gate is running. A new deterministic Codex test retires permission after its validation read and before fork bytes; its negative proof is being captured before the runtime commit. The terminal still embeds its session; do not claim cutover or completion.

Update note (2026-11-14, edit gate passed): all 21 daemon, 183 terminal and 23 native-monitor workflows pass with installed Codex contracts at `target/ui-artifacts/run-yRfXLD`; log `/tmp/carlos-ep019b/edit-terminal.log`. The final additional Codex fork guard test passes at `codex-fork-guard-positive.log` and fails with the old unguarded path at `codex-fork-guard-negative.log`; the production guard is restored. Full workspace tests/strict lint are being repeated for that final test-only addition before commit. No binary was installed. Remaining scope is unchanged; EP019b is incomplete.

Update note (2026-11-14, task-cache prerequisite): the edit path is committed as `7ac8446`. Added independently addressed child caches, matching native replies, per-target release/deadlines and one combined eight-MiB cache budget with an eight-target cap. Four new core tests pass, alongside all existing workspace tests and strict lint. The complete runner passes 21 daemon, 183 terminal and 23 native-monitor workflows, installed Codex contracts, 61 helper tests and 13 architecture checks at `target/ui-artifacts/run-gK5mGB`; log `/tmp/carlos-ep019b/tasks-cache-terminal.log`. The public task facade, independent task-list paging, remaining native controls, shared client, terminal cutover/deletion and final performance/review remain incomplete.

Update note (2026-11-14, direct source windows): committed the independent child cache as `741e9b7`. Added `entry/source/seek`, exact starting offsets and separate ByteOffset/ByteLength types so a future bounded client cache can revisit old source without replaying the prefix. Two new core seek tests and a lossless-byte protocol check pass with workspace tests and strict Clippy; the actual daemon large-body workflow now checks direct tail seek and invalid offsets. All daemon workflows are being run before the next commit. EP019b remains incomplete.

Update note (2026-11-14, source seek passed): all 21 daemon workflows pass at `target/daemon-artifacts/ep019b-seek`; workspace tests and strict lint pass in `/tmp/carlos-ep019b/source-seek-{tests,clippy}.log`. The process fixture now explicitly closes catalogue-read connections instead of waiting for Python's garbage collector. The next work remains the complete typed native facade and pure shared client, followed by actual terminal cutover/deletion and final checks. No owner-review or completion gate has been reached.

Update note (2026-11-14, client groundwork): the approval-order regression now has a negative proof and passes the complete daemon/terminal/native gate at `run-VdDHRd`. A new uncommitted pure `carlos-client` scaffold has a bounded codec, transcript/history window and ordered approval cards; workspace tests and strict lint passed before its latest small extraction, and 15 architecture checks pass. Greeting/reconnect, private operations, body windows, the complete native facade and terminal cutover/deletion remain unfinished. No terminal or owner-review gate is reached.

Update note (2026-11-14, client data tests passed): approval publisher fix is committed as `d6ef2b3`. The shared client codec/fact windows now pass twelve client tests plus workspace tests and strict Clippy (`client-data-{tests,clippy}.log`); the new dependency guard passes all 15 architecture checks. Complete snapshots bridge gaps atomically, late snapshots cannot roll facts backward, history rejects changed unseen rows, and bounded older windows do not splice in a separated live tail. Request bytes are capped while serializing, before a full oversized output allocation. Private reconnect/operations and body caches are next; no terminal cutover is claimed.

Update note (2026-11-14, private client state): client fact groundwork is committed as `2d8b1bb`. Added checked greeting generations, private lease credentials and lossless counters that preserve locally issued but unadmitted ids and never wrap. Added bounded private text/result tracking, exact recovery checks, shared-row echo binding and no resend path on lease loss. Twenty pure client tests plus a real Unix daemon greeting/resume/expired-operation/list test pass with workspace tests and strict Clippy in `client-private-{tests,clippy}.log`; the test creates no native session. All 15 architecture checks pass. Its temporary directory explicitly uses mode 0700, as the daemon requires. Body windows, client orchestration, the remaining typed native facade and terminal cutover/deletion are still incomplete.

Update note (2026-11-14, delegated EP approval): the owner is going to sleep and expressly authorized Codex to auto-approve completed EPs and continue to the next EP. Updated current plan/program/architecture instructions to remove the human pause after passing all checks and direct review. Historical reports and archived plans remain unchanged. EP019b is still incomplete; this grant is not an early approval of unfinished work. Main, remote and installed-binary constraints remain unchanged.

Update note (2026-11-14, required Fable review): the owner added a condition to delegated approval: before every auto-approval, ask Fable 5.2 medium for the same quick whole-roadmap code-quality review used before, then use judgment to fix/change or explain each finding. This explicitly authorizes that read-only reviewer and leaves implementation with Codex. Obtain the review only for a concrete completed EP before approval; current EP019b remains in progress.

Update note (2026-11-14, source roles and formats): private client state is committed as `feef265`. Added an explicit tool_call kind, default plain/user_steer/diff format and optional source file path. Plain defaults stay omitted in old fixture shapes. Source-kind changes that cross the inline/output boundary now replace the row; native failure flags remain visible before completion. The new process fixture proves steered input, tool calls and Unicode diff source/paths through the real socket. The full runner passes at `run-K0tVof`, including 22 daemon, 183 terminal, 23 native-monitor workflows, 15 architecture checks and installed Codex contracts; strict Clippy passes. Existing tool tests now wait for actual has_output facts whether they arrive as an add or a later update, preserving the real tool-call-to-output transition. Next: typed native goal/queue/task/settings controls, then remaining client/renderer/transport work and final deletion.

Update note (2026-11-14, typed goal family in progress): source-format changes are committed as `1bd6834`. Added typed goal facts/read/set/clear, opaque root/connection/content preconditions, shared snapshot/events and private native results. The publisher hashes compact revision/flags before cloning goal text; operation errors stay private. Native goal requests keep the existing tokens/deadlines and own a liveness flag checked by the captured writer. Two real Codex guard tests pass, including callback errors; deliberately bypassing the async guard fails its test. Initial workspace tests pass before the latest guard tests; actual daemon goal workflows are being run and the plan remains incomplete. Further work includes goal race/validation tests, other native families, source/body client caches, renderer/socket cutover, explicit deletion and final review/perf.

Update note (2026-11-14, goal checks): all five new actual-daemon goal workflows pass, including bounds and unsupported Grok controls; the complete daemon count is now 27. Core/client tests cover shared facts, private errors, revision exhaustion and late reads. Equal read replies keep revisions stable, while equal unsolicited native events still fence activation acknowledgements, preserving the existing paused-progress-before-ACK test. The first full terminal gate had one unrelated resize-check race: its in-memory screen already had 28 columns but still displayed the old desktop composer glyph; its text-only wait accepted that frame before the new phone layout arrived. The test now waits for both the title and the correct dim placeholder. Retained artifacts are `run-4QPJ7L`; the strengthened check and full gate are being repeated. Strict Clippy passes, including boxed cold reset/perf snapshot variants rather than making every event carry the full expanded snapshot inline.

Update note (2026-11-14, reply-frame reserve): a full repeat exposed a real outbox gap during Grok history restore (`run-Ss7Rr1`): small shared deltas could occupy all 128 frame slots, leaving no slot for the next required sessions/watch reply even though byte headroom remained. Added a 96-frame shared limit under the existing 128-frame total, reserving 32 slots for replies/recovery snapshots, with a deterministic active-write/count-cap test. No queue or byte budget was enlarged. The strengthened Unicode resize test passes; the full daemon/terminal/native gate is being repeated after the reserve fix.

Update note (2026-11-14, queue reserve verified): all 27 actual-daemon workflows pass after the count reserve at `target/daemon-artifacts/ep019b-reserve`. The deterministic outbox test fails with the old source (`reply-reserve-negative.log`); the fixed source is restored and workspace tests/strict Clippy pass. Direct review: active frames retain their count/byte charge, disposable shared traffic stops at 96 frames/eight MiB, and replies/snapshots can use the existing 128-frame/sixteen-MiB total. No new queue or scheduler was added. The transport now checks every typed golden fixture through the real method/field decoder, preventing a new proto command from remaining silently blocked by the method list.
