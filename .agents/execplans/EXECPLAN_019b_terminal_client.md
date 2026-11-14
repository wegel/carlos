# EP019b: run the terminal through the daemon and delete the embedded path


This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. The owner explicitly accepted EP019a in this conversation on 2026-11-14 and then requested autonomous execution. The owner originally requested direct work, then explicitly authorized one natural developer split in the current conversation: Sol 6.1 High ports the native goal panel while Codex owns large-body/source rendering. Keep these code areas separate and coordinate their shared terminal hooks. The owner separately authorized a Fable 5.2 medium review before each auto-approval; this reviewer may read but must not implement changes. Do not edit archived plans.

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
- [x] (2026-11-14) Add typed native goal reads/changes, shared facts and private acknowledgements, preserving native tokens and guarding actual writer dispatch. Five new daemon workflows pass; the full gate passes 27 daemon, 183 terminal and 23 native-monitor checks at `run-LvqdZ8`, with installed Codex contracts, workspace tests and strict Clippy. The terminal consumer remains unfinished.
- [x] (2026-11-14) Add typed native queue reads/text/changes with private results, bounded previews, opaque selected entries, preserved rich native inputs and guarded request lifetimes. Seven new daemon workflows pass; the full gate passes 34 daemon, 183 terminal and 23 native-monitor checks at `run-tz3na7`. Restored workspace tests and strict Clippy pass. Omitting the revision comparison fails the strengthened real-socket stale-page test. Terminal consumers remain unfinished.
- [x] (2026-11-14) Capture each descendant reader's native root and extract its bounded page parser. Reconnect now drives the same reader without a temporary SessionState. Full terminal/native gates pass at run-bP9fHI and run-2bv1Iv; captured-root negative proof fails with the old root lookup.
- [x] (2026-11-14) Add the native Claude helper run fence and task/run reads. The helper observes run identities before the SDK queue; seven core read tests and four new socket workflows preserve exact current turns, empty/unknown states and reader lifetimes. Full gate passes 43 daemon, 183 terminal, 23 native-monitor and 67 helper checks at run-ymnnH0. Removing read-id checks fails the replacement-poll proof; removing the helper guard sends an extra native stop and fails its SDK proof. Public stop remains unfinished.
- [x] (2026-11-14) Add private exact-run task stops, retaining root/connection write permissions and process PID/item checks. Nine focused core tests and six new real-socket workflows include a full compiled-Claude-helper pipe. Both engineering findings are fixed and the separate follow-up verdict is PASS. Negative proofs fail when worker dispatch or uncertain outcomes are removed. Restored workspace tests, strict lint and the full 49-daemon/183-terminal/23-native gate pass; artifacts are target/ui-artifacts/run-PW0kwe and source logs are task-stop-final-{tests,clippy,terminal}.log.
- [ ] Finish the typed task family. Current list/detail/named-text/current-run/stop routes are checked. Independent older native pages, activity/history and numbered task facts remain open. Add read interests only where an actual consumer needs them.
- [ ] Prove a terminal transcript consumer before adding more task APIs; the stop slice is committed. Fable 5.2 medium reviewed the whole roadmap at this unfinished checkpoint; its report and the assessed actions are `.agents/reports/exec019b-fable-interim-review.md`. No EP approval has occurred. Measure contiguous source reads and real Markdown/ANSI/copy behavior under explicit aggregate memory bounds; choose the render contract from that proof. Then port each remaining panel together with its typed requests.
- [x] (2026-11-14) Commit the checked stop slice as `99d39aa`; main and installed binary remain unchanged.
- [x] (2026-11-14) Prototype the terminal's complete-source render contract. Added a borrowed BodyInput used by the existing body renderer; three tests match old text/styles/wraps and reject suffixes/retired bodies. `scripts/probe-client-render.py` captures large Markdown/ANSI bodies from actual Unix-socket reads; `socket_render_probe` replays those chunks through the current bounded client window and the contiguous-body candidate. The socket replay, default/no-default tests, strict lint and 49-daemon/183-terminal gate pass at run-0UU3za. The separate engineering follow-up is PASS; the focused report is `.agents/reports/exec019b-render-contract-prototype.md`. This is no production terminal socket consumer yet, and no larger cache or product body cap is accepted.
- [x] (2026-11-14) Extend the borrowed input through complete block headings, compact labels and spacing. RenderInput carries borrowed source and label facts; the transcript cache retains stable entry identity separately. DisplayMessage and BlockPlan use it without a core Message copy. An actual exported-row test covers full/compact Gutter/Flow blocks at 12/28/72/120 columns. The test exposed a missing wire fact: a generic fallback label and an explicitly saved label can both read “Tool output” but render differently for diffs. LabelSource::Fallback now preserves that distinction without duplicating the label string. Restored tests, strict lint, no-default tests, the full protocol/terminal/native gate and release measurements pass; the engineering verdict is PASS. Evidence is `.agents/reports/exec019b-render-blocks.md`. The Client-backed terminal connection remains unfinished.
- [x] (2026-11-14) Add caller-chosen contiguous source inside the existing client cache. Source and normalized windows share one eight-MiB retained-data allowance; default 256-KiB suffix behavior and exact revisions/cursors remain. Watch replacement preserves document mode; direct and live capacity failures stay local and preserve other windows/private text. The 58-test pure client suite, workspace/strict lint, real-socket source replay and full 49-daemon/183-terminal gate pass. Separate engineering verdict PASS WITH ISSUES; accepted the missing live-pressure coverage and added it for both formats. Report: `.agents/reports/exec019b-contiguous-source.md`. This cache bound is not an approved product body limit.
- [x] (2026-11-14) Prove the first actual socket terminal consumer through the shared Client, borrowed lazy render cache, composer and ordinary approval painter. The opt-in attach path creates no SessionState or native backend in the terminal. Six screen/RPC tests cover two independent drafts and held-send controls, daemon restart/F5 without resend,130 handled clientId echoes, local cache pressure, a daemon read-RPC fault and a456-KiB ANSI source with exact bytes/red tail. The engineering reviewer found missing failed-RPC token retirement; it is fixed, the negative proof reproduces the extra subscription and the narrow follow-up is PASS. The restored full49-daemon/189-terminal gate, strict lint, no-default tests and release smoke pass. Evidence: `.agents/reports/exec019b-first-socket-terminal.md`. Main and installed binary remain unchanged.
- [x] (2026-11-14) Share local display commands, pointer/page gestures, selected-cell/copy rules and status separator with the socket terminal. New actual-socket checks preserve exact Unicode selection, phone-width ANSI color, offline folding and complete552-KiB Markdown copy without a chat send. Restored full gate passes49 daemon/191 terminal/67 helper/15 architecture checks at run-xB3tHI; strict lint and both engineering reviews are PASS. Matched explicit releases preserve counts and keep p95 near the original/pre-local figures; the stale/mixed-profile probe is excluded and its correction is recorded. Focused evidence: `.agents/reports/exec019b-socket-local-views.md`.
- [x] (2026-11-14) Keep every entry reachable through bounded source reads, actual history paging and eviction. All200 source texts appear while scrolling, the old prefix survives a prepend/resize and watch high water stays32. Delayed live source retains the accepted raster; explicit F6 retries a failed visible read without replaying chat. Reset/empty-source/live-growth/tool-budget regressions each have an actual-socket proof and a failing single-fix negative. A separate embedded prepend+changed-id regression fails before its repair and passes in both themes at12/28/80columns. All reviewer findings are fixed, with final narrow verdict PASS. Exact final gates pass824main Rust/58client tests, strict lint,49daemon/197terminal/67helper/15architecture checks and explicit release. The15-round matched release comparison removes the initial-layout regression and preserves counts/p95; seven-round append comparisons cover1501/24001entries. Evidence: `.agents/reports/exec019b-sparse-source-history.md`.
- [ ] Measure64MiB ANSI/16MiB Markdown source versus rendered-block high water, append/resize p95 and exact above-cache copy before choosing storage or checkpoints. The second Fable5.2medium whole-roadmap report remains `.agents/reports/exec019b-fable-source-cache-review.md`; no file fallback has started and no product body cap is accepted. Use the actual above-cache caller to justify a pure-client pass-through and a single pinned-document candidate; do not build file/mmap or stream Markdown without evidence.
- [x] (2026-11-14) Measure a test-only ANSI source/style checkpoint candidate and repair the measured64MiB single-word prefix walk. Six differential tests, isolated full no-default workspace tests, strict Clippy, exact source/private-copy digests and size checks pass. The engineering verdict is PASS WITH ISSUES with one MINOR in stale copy evidence; unique run files and a successful backend guard fix it, two negative proofs reject stale results, and the narrow follow-up is PASS. The final exact release test is731c23831eee366b7afcfb0703225636110bcc89a1fdd80920e32a985c50c739; report: `.agents/reports/exec019b-ansi-checkpoint-prototype.md`. This closes an offline prototype only; general long mixed-style/Unicode row indexes, Markdown, worker ownership and actual socket above-cache copy/input checks remain open.
- [x] (2026-11-14) Port the existing native goal panel through the daemon with shared widgets, private drafts, budgets/status/clear, frozen revision confirmations and Ralph guards. The Sol 6.1 High worker committed d24366f. Explicit goal read interests preserve the existing native timer across multiple clients and stop it after the final reader leaves; offline and failed-admitted reads are covered. Actual restart tests fixed lower-sequence new-instance snapshots, duplicate subscriptions and early Enter during restore without replaying private mutations. All1270 Rust tests, strict lint and the full49daemon/207terminal/23native-monitor plus installed-Codex/helper/architecture gate pass. The initial engineering FAIL findings are fixed and the final verdict is PASS. Report with both raw verdicts: `.agents/reports/exec019b-daemon-goal-panel.md`.
- [x] (2026-11-14) Port the native Codex queue panel through the shared Client and daemon. The Sol6.1 High worker committed c4bdbca; pages, full selected text, independent editors, F3 Add/private recovery, stale revision/run/root guards and reconnect all pass actual screen/RPC tests. The narrow ordinary-lane repair allows only a head QueueChange beside a pending Prompt, with held rename/fork barriers intact. Initial engineering FAIL findings and the follow-up obsolete-footer MINOR are fixed; final verdict PASS. The exact full contract gate passes1280Rust/53daemon/219terminal/23native-monitor plus67helper/15architecture/package checks at run-cVnMvz. Root inspected the handoff and replaced the string control selector with an exhaustive enum; its repeat gate passes1281Rust/53daemon/219terminal at run-ws9dsg. Evidence and raw reviews: `.agents/reports/exec019b-daemon-queue-panel.md`. Claude queue mode, other panels/startup and the cutover remain open.
- [ ] Complete the terminal consumer beyond this first attach slice. The explicit path currently attaches an existing logical session, keeps at most32 source windows with actual history/eviction, and still needs above-cache fidelity, complete copy/selection/navigation, structured cards, native panels/settings, local commands and startup/create/discovery. Preserve unconfirmed operation identities rather than discard echoes or compare by text. Share existing widgets and remove the old wrappers only after actual socket parity. The default remains embedded until the separate deletion commit.
- [ ] Add typed native facts/commands for names, prompt edits, task reads/stops, queues, goals, settings/context/quota, summaries, host defaults and lifecycle. Preserve existing tokens and target/generation guards; prove two clients can choose different read targets.
- [x] (2026-11-14) Add the pure shared client codec and bounded fact/history/card windows. Twelve client tests, workspace tests, strict Clippy and 15 architecture checks pass. The checker allows only reviewed data packages below the client.
- [x] (2026-11-14) Add private connection credentials/counters and operation-owned drafts/echoes. Twenty client tests, the actual daemon wire test, workspace tests and strict lint pass; 15 architecture checks pass. No client method automatically resends a mutation.
- [x] (2026-11-14) Add bounded original-source and normalized-output windows with separate cursors, retired-read guards, exact UTF-8 byte intervals and explicit missing prefixes. Six new tests pass with workspace tests/strict lint. Removing the source byte bound fails the large-source test; restored source passes. Actual terminal layout/color/copy consumers remain unfinished.
- [x] (2026-11-14) Add one coherent Client for the current wire methods, with correlated replies, automatic snapshot reads after gaps, private outcome queries, source/output watch lifetimes, bounded future-history pages and a combined fact budget. Fifty-one client tests and two actual Unix-listener tests pass; whole workspace tests/strict lint pass. Remaining native families and actual terminal consumers still need to use this interface.
- [ ] Complete `carlos-client` for greeting, private retry rights, numbered facts, history, source/output cursors and operation-owned local echo. Test gaps, replacement, reconnect, malformed input and private recovery without sockets or clocks inside the crate.
- [ ] Route terminal startup and every existing workflow through the actual daemon and shared client. Preserve drafts, dictation, color/copy behavior, panels and startup choices. Add screen/request tests for any broken workflow.
- [ ] After socket parity passes, make the separate buildable commit `tui: remove embedded session and backend path`; remove direct session/backend ownership, temporary adapters and obsolete report consumers. Review that deletion explicitly.
- [ ] Pass final default/no-default tests, strict lint, release/native/terminal/process/dependency checks and matched socket speed/memory comparisons. Review directly, report exact identities/limits, archive the completed plan unchanged, record delegated auto-approval and continue to EP020.

## Surprises & Discoveries

The first ANSI viewport candidate removed retained full-body rows, but a single64MiB word still took1217ms to revisit at28columns. A saved source range for one contiguous styled alphanumeric word reduces that tail read to0.00175ms and append p95 to0.332ms. Ordinary64MiB output retains about75MiB RSS rather than the baseline803300KiB. These are test-only isolated formatter measurements, not actual terminal latency. General long mixed-style/Unicode row indexes, Markdown and ownership away from the input loop still need work; the prototype must not bypass those requirements. Evidence and exact executable identities are in `.agents/reports/exec019b-ansi-checkpoint-prototype.md`.

Task-stop work after commit `95d2d01` captures an opaque task/run pair and a private operation. Root/connection changes retire queued native writes; refreshed process PID/item evidence also retires process stops. A Codex child interrupt already names its exact native turn, and Claude checks the run inside the helper, so Rust no longer duplicates those two run fences. Acknowledgements settle only the originating operation and never invent task completion. Nine focused core tests and six new socket workflows include a deliberately full compiled-helper pipe. The restored full gate passes and the engineering follow-up verdict is PASS.

The engineering reviewer found that Claude's request_native_control waits up to 500 ms for its pipe writer despite returning replies asynchronously. Calling it in the owner loop blocked client reads, and treating every dispatch error as Rejected misstated a possible write. Task stops now use the existing bounded control workers, register the pending outcome before dispatch and report unconfirmed writes as Uncertain. An early native acknowledgement wins over a later local writer error.

The actual-source renderer probe found that tool entries intentionally carry has_output/output_revision rather than Entry.source. A borrowed client body must check that tool revision and the returned source's complete byte length; treating the empty row text as the complete tool body would erase the output. Native command bodies also retain the core's existing command heading and exit footer. The socket probe compares that complete stored source, including those wrappers and all original ANSI bytes. The three local renderer tests show equal whole-body output across Markdown tables/fences, reasoning, ANSI, diffs, source highlighting and narrow widths; a deliberately missing ANSI prefix changes the tail's colors even when the visible tail text is identical.

The Claude helper currently ignores runId when it invokes query.stopTask(taskId). The public stop route must validate the expected observed run at native dispatch as well as in the owner. Codex agent interrupts can name an exact turn; process termination names only processId and needs checked item/PID evidence. Keep acknowledgements separate from task completion. These routes remain unfinished.



The captured Codex async-control and callback-error paths checked the connection at enqueue but bypassed the captured permission at writer dispatch. The new control test passes with the fix and fails when that async path is deliberately restored (`goal-guard-negative.log`). Remote goal commands now retain their own request lifetime, so a fork/close retires a queued write as soon as core drops the request, and the owner returns an uncertain private result rather than leaving the old command pending. All five actual-daemon goal workflows pass; the terminal still needs to consume these commands during cutover.


The terminal still embeds `SessionState` in `src/app/state.rs`, consumes borrowed change records in `src/app/core_events.rs`, and reads native panel state directly. Replacing only the backend trait with socket forwarding would leave a second privileged session and cannot meet the deletion gate. The terminal must render a client-owned fact copy instead.

An old Grok restore check waited for “History complete” inside a wire notification. With the new explicit large-body descriptor, that full text belongs in a source read. The check now waits for Ready and reads the restored source before asserting its contents. A new Codex fixture initially sent an item-completed record without its required final text, which correctly replaced the body with empty text; the fixture now uses the turn-completed boundary after its streamed deltas. No native completion behavior was weakened.

Promoting rewind exposed two native gaps. Captured Codex handles delegated a fork's validation/mutation calls to the unguarded connection; the fork now uses the captured facade throughout. Claude's fork outlives its initial request, so the command's short write permission cannot also represent the entire edit. The adapter confirms that first guarded write before releasing the ordinary slot, while a separate edit lifetime and originating operation protect later helper replacement and failure facts. The core registers the edited draft before dispatch so an early helper switch cannot lose it or restart it on a late write acknowledgement. Three actual-daemon edit workflows pass for names, Codex fork/send and compiled Claude fork/acknowledgement with original native history unchanged.

The old Codex panel retained only one selected child's activity. Its native tokens and deadlines now address a map of at most eight child reads, and all cached child items/tails share the existing eight-MiB budget rather than getting eight MiB each. Replies match original root/epoch/token/method/parameters and cannot consult another client's selected row. Releasing one target prunes only that request. The embedded terminal still clears its own reads on navigation, while the future daemon facade must aggregate explicit interests and release a target only when its last client leaves. The native task-list older-page window also remains global; it must be separated from client choices before exposing complete task paging. Do not mistake the detail-cache prerequisite for a complete public task family.

A bounded client cache also needs direct reads at earlier byte positions. Keeping only a forward cursor would force it to transfer a large prefix again when the user scrolls back. `entry/source/seek` now checks the expected body revision, rounds down to a UTF-8 boundary and returns at most 64 KiB directly from the original string. Source chunks report their exact starting offset without exposing cursor internals. ByteOffset and ByteLength remain distinct types and serialize losslessly as decimal strings. This is a concrete terminal consumer need; it does not add another stored body or parser.

EP019a's normalized tool chunks deliberately strip terminal controls. `crates/carlos-core/src/session/output.rs` reads that normalized view from the single original body. The terminal needs the original bytes for its existing safe color renderer and exact copy, so it must request a separate source format with a cursor that cannot be reused for normalized output. The old eight-MiB snapshot preflight rejects a whole page or large plain row; the new path must bound text independently from metadata rather than raise frame budgets.

Main has not advanced beyond `a29b1a1` at plan start. EP019a's final source is `58e9901`, closeout `614f296`. Its terminal binary is 20,346,176 bytes, SHA-256 `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`. EP019a's raw artifacts remain under `/tmp/carlos-ep019a/`; its tracked report contains the exact methods if temporary evidence disappears.

The shared client exposed a card-order defect: the publisher sent new cards before the new cards used as their anchors, and omitted order-only moves. The publisher now emits from tail to head and compares both card facts and the next-card address. The new deterministic test fails with the old publisher (`approval-order-negative.log`); the full terminal/native gate passes with the fix.

## Decision Log

Decision: measure original-source/style checkpoints and visible rows before changing source storage. Keep complete physical-line parsing and existing natural word breaks; give the measured single-word pathology a direct saved range rather than walk its entire prefix on each scroll. Rationale: painted rows dominated the baseline, while a source mapping alone would preserve that cost. The candidate remains test-only until general long lines, whole-document Markdown, bounded worker ownership and actual socket/selection/copy checks pass. No additional cache allowance, product body limit or generic scheduler is accepted. Date/Author:2026-11-14,Codex.

Decision: fix sparse per-entry render counts, visible loading states, eviction/retry and stable viewport anchors before adding an above-cache file fallback. Rationale: Fable's second whole-roadmap review found that the common 33rd tool source currently disappears and that mmap would not solve whole-block materialization or per-append reparsing. Prove 200 tool rows with actual socket history/scroll/resize first; then measure 64-MiB ANSI and 16-MiB Markdown plus exact above-cache copy to decide whether one pinned document, file/mmap or within-block checkpoints are justified. Keep pure-client cursor/revision/offset checks and admit no product body cap. Date/Author: 2026-11-14, Codex after Fable 5.2 medium.

Decision: add an explicit complete-source lifetime inside the existing Bodies cache and charge source/output under one eight-MiB retained-data allowance. Rationale: the existing renderer must borrow the full Markdown/ANSI source; a second joined body or an unbounded per-entry cache would add copies. Keep ordinary 256-KiB suffix reads and direct seeks. This is a cache bound, not an accepted product body cap. Date/Author: 2026-11-14, Codex.

Decision: prove the terminal transcript/source consumer before adding older task windows, history or another interest mechanism. Rationale: Fable's interim whole-roadmap review correctly identified that source windows alone do not establish whole-document Markdown/ANSI/copy fidelity. Measure a bounded contiguous-body prototype first, then port panels together with their wire requests. Keep the final embedded-path deletion gate and every workflow requirement. Date/Author: 2026-11-14, Codex after Fable 5.2 medium review.

Decision: retain prepare-time task/run validation and native exact-run checks, with additional core PID/item reconciliation only for process stops. Rationale: native Codex turns and Claude helper runs already prevent redirection, while process termination names only processId. Keep root/connection write permissions, private outcome records and the old embedded Claude stop record until its consumer moves, but add no public duplicate stop-state cache. Date/Author: 2026-11-14, Codex after Fable review.

Decision: check Claude's supplied native run id against a bounded table observed before the SDK's unread queue. Preserve terminal tombstones and disable guarded task stops explicitly if that table exceeds its bounds. Rationale: checking only Rust's later copy can miss a replacement already waiting inside the helper; evicting a newer tombstone could authorize a late older run. The table retains identities only, with no task body or new worker. Legacy requests without a run id keep their existing behavior until the public stop adapter can advertise its exact supported guard. Date/Author: 2026-11-14, Codex.


Decision: share the native task parser without creating a session or a new scheduler, and expose current metadata through independent bounded pages and named scalar text reads. Rationale: reconnect and future older-window readers need the same ancestry/size/deadline rules; two clients must not overwrite one shared selection, and a long native command/report must not enlarge a metadata reply. Keep full scalar values in the existing native cache and disclose clipped labels/previews. Date/Author: 2026-11-14, Codex.



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

Decision: hash native goal revision and connection/root scope for edit preconditions, and publish only accepted goal facts and read diagnostics. Rationale: another client's unchanged refresh must not expire an editor, unsolicited native events must still fence racing activation acknowledgements, and mutation failures belong to the submitting operation. Date/Author: 2026-11-14, Codex.

Decision: keep native queue inputs in the existing bounded core snapshot and expose preview pages plus separate revision-checked plain text chunks. Rationale: persisted inputs can be much larger than a safe page and may contain attachments/unknown fields. Each client pages by opaque entry id without a shared selected row. Typed moves/starts/deletes preserve the native input; typed edits require the complete supported plain text shape. Preserve the existing refusal to rewind while a queue write is unconfirmed. Date/Author: 2026-11-14, Codex.

Decision: keep source and normalized client body windows under separate cursor types and explicit read lifetimes. Rationale: revisiting a raw byte range must not apply replies from a discarded window, and normalized pieces cannot supply original terminal controls or exact copy text. Retain at most 32 windows with 256 KiB of text each and bounded piece counts; disclose discarded prefixes and issue explicit seeks rather than silently treating a suffix as the whole body. This does not yet settle terminal Markdown/ANSI layout across arbitrary ranges. Date/Author: 2026-11-14, Codex.

Decision: correlate requests inside one pure Client and activate each body watch only after its first successful reply. Rationale: the server queues the first reply before that watch's notifications; an old reset arriving while a replacement is pending must close the old local token, not the replacement. Explicit close cancels even a pending first read, and reconnect restores checked reads without repeating mutations. Date/Author: 2026-11-14, Codex.

Decision: cap retained session facts across one Client at 64 MiB and defer future history pages under one eight-MiB/32-page allowance. Rationale: multiplying each session's standalone budget by 32 would retain too much client data. On overflow, detach only the overflowing fact window and retain private submitted text and backend work. A history RPC can arrive before its numbered events; hold the fixed page until that cursor arrives instead of repeatedly reading a continuously advancing backend. These data-byte counts include fixed metadata allowances and do not claim exact allocator/RSS measurements. Date/Author: 2026-11-14, Codex.

## Outcomes & Retrospective

The opt-in socket terminal now preserves local display commands, sparse history, bounded source
reads and the native goal panel. The Sol 6.1 High worker is finishing the independent native
queue panel and its daemon ordering regressions. Root measured actual64MiB ANSI/16MiB Markdown
sources and the old full-raster cost, then wrote a test-only ANSI checkpoint candidate.
Its ordinary output and saved single-word viewport results are promising, but general long
lines, whole-document Markdown, actual above-cache ownership/copy and input-loop checks remain
open. The complete terminal cutover, other native panels/settings/startup/dictation and final
embedded-path deletion remain open. The owner has not auto-approved EP019b; the next Fable
review belongs at actual EP completion. Main and the installed binary remain unchanged.


Current task groundwork captures each native root, reuses one bounded parser during reconnect, and serves typed current-page metadata through the actual dispatcher. Four core tests and five socket workflows cover independent client targets, stale preconditions, run replacement, exact large Unicode fields, unavailable scopes and held reads while priority controls advance. These checks do not prove the remaining history/older-window/watch routes or the terminal cutover. Fable's interim whole-roadmap review is recorded below and in its focused report. No EP auto-approval has run.



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


Extend typed protocol modules by concern rather than enlarging a single command file. Add `session/rename`, `session/rewind` with a stable prompt entry target, task list/detail/history/stop requests with opaque run-bound addresses, native queue read/mutations with expected revisions, goal reads/mutations, runtime settings/capabilities, detailed context/quota/compaction, summary settings and host defaults. Add native saved-session discovery/resume and explicit lifecycle controls needed by current CLI startup. Never accept arbitrary backend method names or raw native parameters from clients. After the current stop slice, alternate this work with the terminal consumer in Milestone 4: prove transcript/send/approval/source rendering first, then add or adjust each remaining panel family only while porting its actual workflow. Do not finish every speculative server interface before starting the terminal.

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

Earlier checkpoint notes, native negative proofs and all prior raw-evidence paths are
preserved unchanged in `.agents/reports/exec019b-checkpoint-log.md`. Read the current
Progress and Decision Log above for the next work; the report records earlier states.


## Review: task-stop slice

Reviewer prompt: `.agents/reviewers/engineering_reviewer.md`. Two bounded separate Codex
sessions reviewed the code without changing it. The initial executable sandbox attempt
could not inspect files; a rerun with command access established the findings below.

### Initial verdict

Reviewed all tracked and untracked changes against `95d2d01`, including native guards, daemon/Core/Claude replies, and task invalidation hooks. The seven focused core tests passed.

1. **MAJOR: Claude stops block the session owner.**
   [task_stops.rs:57](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/engine/task_stops.rs:57) calls `work.dispatch()` directly. For Claude, that reaches `ControllerTransport::send()` and `InputWriter::write_guarded()`, which waits up to 500 ms for stdin. During that wait, the owner cannot serve reads, process approvals, or apply native events. This violates the explicit requirement that the owner never wait on native I/O.

   **Corrective guidance:** enqueue the guarded write without waiting, then report its result asynchronously. Preserve the eight-stop limit and writer-time identity check. Add a daemon test that stalls helper stdin and verifies that reads and controls remain responsive.

2. **MAJOR: An unconfirmed write becomes a definite rejection.**
   [task_stops.rs:68](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/engine/task_stops.rs:68) converts every dispatch error into `Rejected`. Claude’s writer timeout explicitly allows that the request may already have been accepted. A delayed write can therefore stop the native task while Carlos reports rejection; any later acknowledgement is ignored because Carlos never retained the pending stop.

   **Corrective guidance:** distinguish failures proven to occur before any bytes were written from failures after a write may have started. Report the latter as `Uncertain`. Add a test covering a delayed write whose native result cannot be confirmed.

**Verdict: FAIL**

### Corrected follow-up verdict

Reviewed the two fixes, worker-slot lifetimes, pending records, native ACK ordering, and the guards that prevent stale writes.

Both prior MAJOR findings are resolved safely:

- The owner sends native writes through the existing four control workers. It installs each pending record before dispatch and releases each worker slot on `TaskStopDispatch`, even if an ACK already removed the record.
- An unconfirmed write returns `Uncertain`. A late writer error cannot overwrite an earlier native ACK.
- Root and connection guards remain intact. Prepared commands retain the exact native turn or Claude helper run; process claims still check PID/item identity.

I reran the nine focused core tests; all passed. The supplied compiled-helper pipe-full test and negative proofs directly support both fixes.

No BLOCKER, MAJOR, or MINOR findings within this scope. No corrective changes requested. This verdict does not cover the roadmap or the still-running full gate.

**Verdict: PASS**

The focused report is `.agents/reports/exec019b-task-stop-engineering-review.md`.
Fable’s whole-roadmap interim report and the assessed actions are
`.agents/reports/exec019b-fable-interim-review.md`; it did not approve the unfinished EP.

Update note (2026-11-14, interim review and stop corrections): record the two engineering
findings and passing follow-up, remove redundant native-run reconciliation, and place the
terminal source/render prototype before further task API work. Keep the full parity and
embedded-path deletion gates. Final restored workspace/lint/full checks pass. The stop slice
is ready to commit; EP019b is still incomplete and has not been auto-approved. Main and the
installed executable remain unchanged.


Update note (2026-11-14, renderer prototype review): the first whole-body socket captures
pass for552059-byte Markdown and456047-byte ANSI bodies through9/7 bounded reads. Equal
render fingerprints compare the two adapters through refactored helpers, not an independent
old executable. The reviewer found a real tool-call shape the first fixture missed: those
rows can have no descriptor, empty inline text and has_output=false while still owning
source. The constructor now requires verified source for every tool row, and a new test uses
the actual core-exported shape at28/72/120cols. Follow-up review remains required. The first
full runner passed49 daemon workflows but one of183 terminal checks sampled the status row
partway through a resize: the context text arrived before its percentage. The Ralph scanner
fixture now waits for the same complete resized right edge that its sibling tests already
require, before asserting that colors animate while text stays fixed. This does not alter the
application or weaken screen/request assertions. Targeted and full reruns are required.


## Review: borrowed-source renderer prototype

The bounded read-only reviewer used `.agents/reviewers/engineering_reviewer.md`.
The initial review found that real tool calls without has_output/Entry.source lost their
separately stored body. That finding is fixed with an actual core-exported row test.

Reviewed the scoped diff against `99d39aa`, including borrowed lifetimes, count/render branches, client mappings, and socket replay. The four focused tests and captured socket replay pass. I found no accidental production count/render change or text copy.

- **MAJOR: The client constructor silently discards tool-call source.** At [transcript_input.rs:41](/var/home/wegel/work/perso/carlos-daemon/src/app/transcript_input.rs:41), the constructor requires external source only when `tool.has_output` is true. The core exports ordinary tool calls with empty `entry.text`, no `entry.source`, and `has_output: false`. Consequently, even when the caller supplies complete source such as `"Bash\nprintf 'hello'\n"`, the constructor ignores it and returns an empty body. It also accepts missing source as an empty body. The current tool-call test misses this because its helper supplies a source descriptor that real tool-call rows lack.

  **Corrective guidance:** Handle tool-call source explicitly, checking the tool revision and complete byte length before borrowing it. Reject missing source when the tool-call body must be fetched. Add a test using the actual exported row shape and compare counts, text, styles, and wraps at the stated widths.

The socket replay supports the reported large Markdown/ANSI cases and suffix rejection. Both renderer paths use the refactored helpers, so their equal fingerprints establish agreement between the adapters, not an independent comparison with the renderer at `99d39aa`. They also do not establish terminal cutover or release performance.

**Verdict: FAIL**

Corrected narrow follow-up:

Reviewed `BodyInput::from_client` and the new test using a core-published tool-call row.

Findings: no BLOCKER, MAJOR, or MINOR issues in this follow-up. The constructor now requires source for tool rows even when `has_output=false` and no descriptor exists. That path checks the tool’s output revision and exact source byte count.

The new test reproduces the previously broken row shape, rejects missing source, and compares nonzero counts plus complete rendered text, styles, and wraps at 28/72/120 columns against the original core message. I ran that test; it passed.

Corrective guidance: none required for the prior finding.

**PASS**. The prior MAJOR is resolved.

The focused report `.agents/reports/exec019b-render-contract-prototype.md` records the
actual socket captures, fingerprint scope and limits. Measurements of complete body rendering
are not claims of terminal cutover. Full terminal checks and matched release figures remain
in progress at this checkpoint.


Update note (2026-11-14, resumable plan): move the earlier checkpoint timeline unchanged
to `.agents/reports/exec019b-checkpoint-log.md`, keeping every old proof and discovery. The
current plan retains its purpose, current progress, decisions, complete milestones, acceptance
gates and reviewer output. Remaining performance proof must include repeated large task-field
reads alongside priority controls, as well as final actual socket/client/render speed and RSS.


Update note (2026-11-14, borrowed renderer checkpoint checked): five focused render tests
and the actual-source socket replay pass; the engineering follow-up resolves its tool-call
finding with PASS. Default/no-default workspace tests, strict lint and the full49-daemon/
183-terminal runner pass at run-0UU3za, after the resize fixture fix287a0ac. Matched31-round
perf-session samples preserve original/EP019a/current row counts: current synthetic p95
scroll/typing/append is0.82/0.62/0.67ms. Post-exec VmHWM is24436KiB, versus23732original
and24844EP019a; the focused report explains sampling and the inherited wait4 memory floor.
These remain embedded-terminal checkpoint figures. Neither a full-body product cache limit
nor terminal socket cutover has been accepted. Next build the actual Client-backed transcript
consumer with one aggregate source budget, then prove live replacement/copy/selection and
port each panel with its typed requests. Keep larger-than-cache bodies explicit and resolve
their fidelity before the final deletion. Main and the installed executable stay unchanged.


## Review: complete borrowed transcript blocks

The reviewer used `.agents/reviewers/engineering_reviewer.md` in a separate bounded
read-only session against61b8748. The reviewer compared the changed branches with that
commit rather than treating agreement between two adapters as a historic-renderer proof.

Reviewed the diff against `61b8748`, including the old render branches, borrowed inputs, label origins, failure/generated flags, spacing, row counts, and related tests/docs.

No `BLOCKER`, `MAJOR`, or `MINOR` findings. The core adapter preserves the previous branch behavior. The client adapter correctly distinguishes fallback labels from saved literal labels. Both adapters borrow body, path, and saved-label strings without copying them into another `Message`.

Validation: `cargo test --workspace` passed with 1,260 tests passed and 4 ignored. The new whole-block tests passed across the specified widths, themes, and compact/full modes. Those tests compare adapters through shared code; they do not independently prove equivalence with the historic renderer. I checked the changed branches against the base commit directly.

No corrective changes required.

**Verdict: PASS**

Update note (2026-11-14, full block checks): the actual exported-row comparisons pass for
Gutter/Flow, full/compact and12/28/72/120columns. Removing only fallback-label handling makes
the client show “Tool …” where the core shows the diff path, proving the missing origin
fact matters. Restored workspace tests and strict Clippy pass. Full protocol/terminal/native
checks pass49daemon/183terminal/23native workflows,67helper tests,15architecture checks and
installed Codex contracts at run-U0djcN. No-default tests and the repeated socket source replay
pass. Release build/paired measurements are being captured before the slice commit. Stable
entry IDs remain in the transcript/cache, and formatting views only borrow display facts.
There is still no production socket terminal or product cache limit; EP019b is incomplete.


Update note (2026-11-14, complete borrowed blocks verified): the full gate passes at
run-U0djcN and the separate engineering review is PASS. The source-prefix replay still
passes. The31-round release comparison against original, EP019a and the prior body-adapter
binary preserves all counts; current synthetic scroll/typing/append p95 is0.81/0.61/0.66ms
with24364KiB sampled VmHWM. Detailed identities, raw-evidence paths, limitations and verdict
are in `.agents/reports/exec019b-render-blocks.md`. Next implement the actual client-backed
transcript and socket driver, including aggregate source bounds; no EP approval is claimed.


## Review: contiguous client source cache

Reviewed the diff against `a11d3ed`, including all untracked Rust files. Checked shared cache bounds, suffix trims, source completeness, cursor guards, watch replacement/failure paths, and probe borrowing. `cargo test -p carlos-client` passed all 57 tests.

- **MINOR:** [client_body_pressure.rs:23](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-client/src/tests/client_body_pressure.rs:23) tests capacity failure on the first subscription reply, but never after a watch becomes active. That leaves the separate notification failure path untested. Add a test that accepts the first chunk, exhausts the shared allowance, then sends a live update. Assert that the client closes the failed window, sends one matching unsubscribe, ignores late chunks, preserves unrelated windows and submitted text, and never resubscribes automatically. Cover normalized output as well.

I found no blocking correctness defect. I did not run the captured source replay or workspace-wide checks.

**Verdict: PASS WITH ISSUES**

Accepted the MINOR finding and added both source and normalized live-notification pressure proofs. The failed watch alone stops once; late chunks cannot restart it, and another document plus the exact private prompt survive. The 58-test pure client suite, workspace tests, captured-source replay and strict lint pass. The full 49-daemon/183-terminal gate, helper/architecture checks, release build and a 1000-turn perf-session smoke pass. No binary was installed. Focused report: `.agents/reports/exec019b-contiguous-source.md`.


## Review: first socket terminal consumer

1. Reviewed the new remote modules, direct client/render seams, terminal tests, and saved test logs. The logs confirm the reported checks passed; I did not rerun the full suite.

2. **MAJOR: daemon-reported source errors bypass failed-read bookkeeping.** In [client_replies.rs](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-client/src/client_replies.rs:211), the client closes a failed watch without adding its token to `effects.bodies`. The terminal therefore leaves `Read.failed` false. On the next append to that entry, [bodies.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/bodies.rs:41) sees the missing body and opens another watch for the same failed revision. Repeated appends repeat the failed reads. The passing capacity test exercises a different branch, which does emit the retired token.

3. Emit the retired token when a watched source request returns an RPC error. Add a test that returns that error, appends three times within the same revision, and asserts that the terminal sends no further source subscriptions while its composer remains usable.

4. **FAIL**. Resolve the incomplete source-failure handling before proceeding.

Accepted the MAJOR finding and added the retired token to the failed watched-RPC branch. The actual terminal test injects that first RPC error, observes three real native appends to the same running source, checks no further source subscriptions and keeps a draft usable. Negative proof and restored final checks/follow-up are pending. The earlier broad gate passes185 terminal workflows; this remains an unfinished consumer checkpoint. Full context and exact limitations are `.agents/reports/exec019b-first-socket-terminal.md`.


### Corrected first-consumer follow-up

1. Reviewed the failed watched-RPC branch, relay, focused UI test, and saved positive and negative evidence.

2. No remaining BLOCKER, MAJOR, or MINOR findings within this follow-up. The added token marks the terminal’s read as failed after the client closes its watch. Saved traffic confirms three appends to the same entry at revision `0`, two total source subscriptions, and a usable composer. Removing the fix produces the reported third subscription.

3. No further correction required for the prior MAJOR finding. I did not rerun tests or assess full EP parity.

4. **PASS**.

The real-terminal negative proof fails with a third source subscription when the retired-token line is removed. The restored final full gate passes 49 daemon, 189 terminal, 67 helper and 15 architecture checks at run-CGmxBX, including all six socket terminal workflows. Restored strict lint and release perf-session smoke also pass. No EP approval has occurred.


## Review: local socket display and clipboard

Reviewed the diff against `a3a7d4c`, including the new gesture helpers, socket navigation, local commands, resize anchors, clipboard paths, and terminal tests.

Findings: No `BLOCKER`, `MAJOR`, or `MINOR` issues found. The extracted helpers preserve the previous pointer behavior. Ctrl+Y borrows complete source text, and pointer copy uses the existing wrapped-row and Unicode extractor.

Corrective guidance: None required for this checkpoint.

I relied on the supplied test results and checked the code directly; I did not rerun the full gate. History paging and source eviction remain outside this checkpoint.

Verdict: **PASS**

The reviewer checked the shared gestures/parser/clipboard and resize source directly but relied on supplied test statements. That prompt incorrectly stated both new targeted workflows had passed: the Markdown copy had passed, while the final offline-fold assertion exposed a missing phone status separator. Folding and input clearing had occurred, but the existing terminal checker correctly required the same visible separator as the old phone UI. The corrected path now shares the separator painter in render.rs; the reconnect hint also starts with F5 reconnects to fit 28columns. An obsolete AppState scroll wrapper was removed after strict lint found it unused. The source builds; restored targeted checks and the complete gate must be read before committing this slice. The first gate was built before the shared-separator correction and is not final evidence. New source files are mouse_gestures.rs and remote/{navigation,local_commands}. The next substantial step after this local slice is actual client history/source-cache paging, including capturing a manual-scroll anchor before applying a history prepend; current resize anchors only cover loaded rows.


Release evidence correction: the plain test runner stages debug binaries in its package layout and does not rebuild target/release. The previous3bc054a3 release predates the first socket consumer, so its old smoke figures cannot verify a3a7d4c or the current local-view changes. The full 191-workflow gate and strict lint are green; matched release builds/comparison are now running before this slice commits. The mixed-profile probe remains under /tmp/carlos-ep019b/socket-navigation-latency-mixed-profile and is excluded from release claims. The focused first-consumer report preserves its earlier hashes plus this correction.


The restored local-view targeted tests now both pass, followed by all191 terminal workflows,49 daemon workflows,67 helper and15 architecture checks at run-xB3tHI. Strict lint is green. The narrow separator/footer engineering follow-up is PASS. The independent200-tool native-history probe confirms the Fable finding: snapshot100 metadata rows,32 source subscriptions, earliest visible source168, source100 absent and no history reads. Fix that common-case source/history path next; no file fallback has been started.


Matched release builds now exist for a3a7d4c (167dcc89541535fb908c8fa37ae1d1da12a283cae7b45c08174f72fd23b271cf) and the local-view source (869292a589a3d048c36af480821145f7ae10b2419e62a6cc4c52aef5fad1517f), preserved separately under /tmp/carlos-ep019b. A15-round CPU6 paired comparison preserves all counts and keeps synthetic scroll/typing/append p95 at.83/.62/.67ms for local views versus.83/.62/.66ms before them. However, sharing a Cargo target directory across the baseline worktree left the primary target/release executable pointing at the other worktree despite an up-to-date build. Generated release artifacts for the five workspace packages are now being rebuilt cleanly in the active source; dependency/Whisper artifacts were retained. Recheck exact hashes before committing the evidence. No source or installed binary was removed.


The active-source clean release rebuild reproduces869292a589a3d048c36af480821145f7ae10b2419e62a6cc4c52aef5fad1517f exactly. Correct15-round matched release evidence is now in the local-view report: synthetic current p95.83/.62/.67ms against pre-local.83/.62/.66ms, plus160KiB post-exec high water; all counts match both source baselines. The final whole-socket/deletion memory gate remains open, including the rise that predates local views. This checkpoint is ready to commit; no EP approval or archive is implied.


Sparse-cache work in progress: the cache now stores each entry's count, plan and painted rows under its stable id, revision and display choices, separately from source-window ownership. The existing823 Rust tests pass after replacing mutable parallel index arrays with one chronological row index. This uncommitted change still needs actual visible-source reads, eviction, stable pre-merge scroll anchors and history requests, plus the200-tool screen/RPC proof and a separate engineering review. No additional EP has started or been approved.


The first sparse-cache engineering review returned FAIL with three MAJOR findings: a reset reused old same-id text, an evicted zero-height entry could never request its later nonblank source, and a growing visible watch bypassed byte eviction. Repairs now invalidate every memo on a replacement snapshot, give stale unloaded entries a positive estimate, and evict unused body windows before the pure Client accepts a live source chunk. The actual reset/growth proofs pass; the empty-source proof also passes after waiting for the terminal's numbered event and scrolling to the prefix while the append is current. Failed initial fixtures used completed native history, whose late deltas Carlos correctly ignores; the repaired fixtures publish unfinished native items. One six-MiB source fixture also exposed slow handling of an artificial multi-megabyte single word; the byte-pressure proof now uses ordinary fenced rows, while the pathological long-line cost remains context for the whole-block measurement gate. Explicit source retry is checked with one failed visible source, avoiding an unrelated offscreen failed history row. All three repairs still need negative proofs and a narrow engineering follow-up before this slice commits.

The first fifteen-round release comparison kept scroll/typing p95 steady but added about2ms to initial layout of6001entries. Do not accept that regression as an approved checkpoint: compact the saved heading facts and reserve map/index space before repeating the matched comparison. The old prototype kept glyph/label string pointers and colors in every current/previous context; these facts can be represented by a small typed section kind and resolved to display strings only when drawing a heading. No final EP approval or storage fallback has occurred.


The narrow engineering follow-up accepted the reset/zero-height repairs and compact heading facts, but returned FAIL for another MAJOR byte-accounting defect: tools omit Entry.source, so summing only metadata counted retained tool bodies as zero. The terminal now reserves the larger of a cached source descriptor and current row metadata for every retained source, including tools. Its preflight also reads small size/address hints from an initial source reply, without copying the body or accepting unvalidated bytes. The pure Client still checks the request/token/revision/offset and data bounds. A new actual-daemon variant uses an unused6MiB tool source while a visible answer grows beyond2MiB; it passes together with the other12socket checks. Removing just this cached-descriptor charge makes that new test fail. The three earlier single-fix negative proofs also fail as expected; exact restored sources rebuild afterward. Fresh review of all repairs remains required.

The first compact cache still spent about1.3ms more than the old cache on initial6001-entry layout. Do not waive that performance gate. The cache now keeps each memo beside its stable id in chronological order, reuses an unchanged order without hashing, and creates a temporary id lookup only for a prepend or reorder. Changed entries use the known checked position, with an id fallback while facts are ahead of the last drawn index. Each memo owns one plan directly rather than allocating/sharing a plan that only that memo needs; drawn line vectors remain shared. All prior identity, missing-source and loading rules stay in place. Measure this ordered version and review it before committing the slice.


## Sparse cache engineering reviews

The complete read-only verdicts and concrete repairs are copied into `.agents/reports/exec019b-sparse-source-history.md`. The first review and first follow-up were FAIL for reset/zero-height/live-growth/tool accounting; all four cases now pass actual socket tests and each fails if its own repair is removed. The ordered-cache review was PASS WITH ISSUES for a known-new full scan. A later narrow follow-up caught a separate MAJOR in the temporary embedded adapter: walking only oldlen NEW facts could miss a changed old id after a prepend. A new Rust regression failed8versus16rows before the fix. The adapter now invalidates the saved OLD suffix directly; pure appends visit zero old entries. The final narrow reviewer independently ran that regression in Gutter/Flow at12/28/80columns and returned PASS. No finding is deferred. The final complete source is now running full tests, strict lint, explicit release and matched perf again before this slice commits.

The latest headroom comparison before the old-suffix fix preserves6001/39347/6000 synthetic counts and keeps layout/replay14.98/47.11ms, scroll/typing/append p95.83/.62/.68ms. All paired median append changes are0.00ms with95% intervals spanning zero. Extra7round pairs at1501/24001entries keep append p95near.7ms and show no full-cache append scan. These numbers and the final artifact identities must be recaptured for the exact committed source. No EP approval is implied; large-body and complete-terminal/deletion work still follows.


The sparse-source/history slice is complete and reviewed; it is not EP019b completion. Final exact release SHA is a32f2fa5a594f2b0da91bbc0941f1c0af7f2f87c9fe90980070f1e3b2361f8a1. The accepted fifteen-round matched run preserves all counts; current synthetic layout/replay14.92/47.20ms and scroll/typing/append p95.82/.62/.67ms match the three baselines within the paired measurements. Sampled current high water25532KiB is1512KiB below pre-slice. Seven-round250/4000turn comparisons retain approximately.7ms append p95; paired intervals include zero. Full runner now passes49daemon/197terminal workflows,824main Rust/58client tests,67helper and15architecture checks; strict lint/release/size checks pass. The final narrow old-suffix reviewer is PASS, and its new regression was demonstrated failing8versus16rows before repair. The report records exact identities, all prior verdicts and negative proofs. Main/installed binary are unchanged. After this reviewed slice lands on the spike branch, measure actual 64 MiB ANSI and 16 MiB Markdown plus exact above-cache copy before selecting a production storage/render path. No EP approval or archive has occurred.


Started the large-body measurement after committing the reviewed source/history slice as95d6a73. Measure the existing renderer first using actual daemon-captured64MiB ANSI and16MiB Markdown. A test-only pinned-string candidate may accumulate chunks only after the existing bounded suffix window validates each cursor/revision/offset; it must not raise the production eight-MiB cache cap or claim to be the production pass-through. Record source/raster capacities and process memory separately, plus actual formatter append/resize p95 and a private clipboard-sink digest. Keep this probe separate from production source/storage work. The owner asked whether the repeated reviews happened at EP completion; answered that both earlier Fable reviews were interim and the cascade was engineering review of an unfinished slice. Reserve the next Fable review for actual EP019b completion; continue the authorized EP.


The owner asked whether independent developer work would help and authorized Sol 6.1 High workers only where natural. Assigned one bounded worker, /root/daemon_goal_panel, to port the existing native goal panel using existing typed Client/daemon APIs and actual screen/RPC tests. Root retains source/body/render caches and the large-source probe. The worker may own goal-specific files and notify root before goal-only edits to remote.rs/events/render/local_commands/drafts/test_workflows; root avoids those shared hooks until handoff. No extra implementation worker is assigned. Root owns this plan, final integration/review and all EP approval gates. The next Fable review still belongs at actual EP019b completion.


Large source capture succeeded through the unchanged release daemon:64MiB ANSI in1024source reads (SHA2561845226d523271fc15e979d320742aa0f2d29d558c6be8eb350bce39bcbbdf9d) and16MiB Markdown in257UTF8-safe reads (SHA256f7b91e7d09fe28d6b65a17bab87c67cdf2b467c08a76c21a0b076c0f0975b578). These are quiescent current bodies in an unfinished native turn, not final native completion records: a complete64MiB command item would exceed the native8MiB frame cap, and the running original contains raw output without the completed command header/footer. Initial smoke fixtures incorrectly waited for an Ok tool status and assumed that completed-source wrapper; they were corrected before measurement. Do not claim completed64MiB native items or production above-cache support from this capture. Exact wire digests and daemon identity are in target/large-source-probe/manifest.json; raw logs are /tmp/carlos-ep019b/large-source-capture.log.

The test-only formatter probe is being built at a detached95d6a73 snapshot, /tmp/carlos-ep019b/large-probe-checkout, with its own CARGO_TARGET_DIR=/tmp/carlos-ep019b/large-probe-target and --release --no-default-features. This avoids compiling half-written goal-panel hooks in the shared primary checkout and does not reuse the shared target that previously caused stale release identities. Only root's probe files are copied there; all production edits/commits remain in carlos-daemon. Record the probe's separate artifact hash and feature profile. Its renderer code has no dictation feature branch; source/raster memory is measured within one isolated process. The primary default-feature tests/Clippy passed for the new ignored probe before the worker's subsequent goal edits. The probe still needs a successful smoke run, full measurements and an assessed report.


The large-source measurements are complete in `.agents/reports/exec019b-large-source-baseline.md`. At28columns the64MiB ANSI body creates3,195,660 owned rows, about620MB of Vec/String capacity and803300KiB process RSS after its first raster; append/resize p95 exceed7seconds in this offline full-raster candidate. The16MiB Markdown body creates629146rows and roughly150MB of row capacity, with append/resize over1second. Exact original clipboard digests pass for both sources and widths through a private sink. These are isolated --release --no-default-features formatter measurements, not actual socket latency or production above-cache support. A source mapping alone cannot fix this dominant painted-row cost. Next retain only viewport rows, index/revisit ANSI/plain/diff with style state, keep whole-document Markdown semantics and measure parsed-document memory. Inspect accepted suffix pieces as an existing validated platform source before adding another client API. No file worker/mmap or generic scheduler is justified by these results. Root still owns this lane while the Sol worker completes the goal-panel read lifetime/reconnect slice.


## Goal-panel engineering review

The independent developer handed all shared hooks/client files back at commitd24366f. Root inspected the scoped commit and kept production source work out of its timer/read-state changes. The initial read-only verdict was FAIL for offline desired-read intent and admitted-subscribe cleanup; both have actual relay/daemon tests. The final verdict is PASS without findings and independently ran63client/fivecore goal tests. Full raw outputs and concrete repairs are copied in `.agents/reports/exec019b-daemon-goal-panel.md`. The full contract gate passes49daemon/207terminal/23native-monitor workflows at run-lY0EoZ; final explicit read-retry proof passes separately at goal-failure-retry-final. This closes the goal-panel slice, not EP019b. All other panels/startup/lifecycle/dictation, large-body production work and the separate deletion gate remain open.

The large-source baseline engineering verdict is PASS WITH ISSUES, with two MINOR measurement caveats: total formatter time includes one /proc status read after counting, and the Markdown corpus is mostly a large code fence. Both are stated in `.agents/reports/exec019b-large-source-baseline.md`; do not invent a diagnostic correction or generalize these samples to rich many-element Markdown parsing. The reviewer independently verified every source/copy digest, bounds and sample count. Proceed to bounded row materialization; no Fable/EP approval has occurred.


After the reviewed goal commit, reused the same idle Sol 6.1 High worker for the native queue panel. Existing typed queue APIs and separate queue widgets make this the next independent consumer; no additional worker was added. It owns queue-only remote/client hooks after notifying root, while root still owns source/format/cache files and active plan/reports. Any actual API gap is reported before widening the protocol. No queue code is complete or accepted yet.

Root started a test-only ANSI checkpoint candidate after the full-raster baseline. It saves original byte positions, wrapped-row counts and ANSI style every ordinary bounded batch; a visible window reparses only intersecting batches. The initial private16KiB logical-line restriction was removed. Six differential tests now preserve text, styles, row flags, Unicode, tabs, CR/LF, cross-batch resets, append and resize. A synthetic SGR prefix and next-line sentinel preserve upstream parser behavior, including control-only trailing style changes. The saved single-word range fixes the measured64MiB prefix walk; general long lines still need a saved row index before promotion. Exact captures/private-copy digests, rejected and final measurements, validation scope and open production work are in `.agents/reports/exec019b-ansi-checkpoint-prototype.md`. The engineering review and narrow copy-evidence follow-up are complete; the final verdict is PASS and raw outputs are copied into the report. No production source path, larger cache, file mapping or Fable review was added.

Committed that test-only proof as bf941ee. Its pre-commit hook also passed the full primary workspace tests against the then-current combined working tree. Root next measures the existing whole Markdown parser, its retained logical lines and a compact row prefix index; no streamed Markdown parser is added. A new normal test matches the old renderer across late reference definitions, tables, nested lists/quotes, code fences and narrow widths. A real4MiB rich Markdown capture (headings, inline styles/links/code, nested lists, quotes and tables) arrives in65UTF8-safe source reads, SHA25606af865e844ab8ac58b621d13f6fc2db24e95d73601f9fa366fd4063e1c04872. This supplements the earlier mostly-fenced16MiB corpus; it does not replace or generalize that baseline. Isolated release build is running before full-raster versus parsed-document measurements. Root still owns all transcript/probe files; the Sol worker is finishing the queue panel's final exact-source gate after its engineering follow-up passed.


After queue handoff, assigned the same idle Sol 6.1 High worker the next independent task-panel consumer. It inspected current F2 workflows and reported concrete client/watch/older-window/activity gaps before any protocol edit. Approved buildable reviewed stages within EP019b: A latest native list/detail/current-run/stop plus narrowly required overview and visible-list interest; B independent older native windows; C independent child activity/history and required F2/F4 question-editor parity. Reuse existing tokens/deadlines/timers and one aggregate budget; retain exact stop/run/PID/root/generation fences and opaque named fields. Broader settings/browser/startup/EP021 features remain separate. Root reserves source/body/cache/format/probe files; shared hooks/client/recorder edits wait for root's typed-queue gate/commit. No extra implementation worker was spawned and no Fable/EP approval occurred.
