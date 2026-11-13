# EP018a: prepare the core and output streams for the daemon


This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current.

## Purpose / Big Picture


A client must be able to open a running tool and receive new output without repeatedly fetching its growing body. The daemon must also serve several clients without choosing which client's composer to inspect or overwrite. This plan addresses those requirements before EP019 puts the core behind sockets. It inventories the terminal's existing controls, removes client draft decisions from the session owner, and reduces obsolete boundary machinery. The terminal keeps its current screens and backend behavior throughout.

The owner explicitly approved this broader scope after Fable 5.2 medium reviewed the whole iPhone/daemon roadmap. The owner also superseded the old phone prototype's restriction on expanding running tools: the engine protocol must support that workflow now, even though EP022 will build the phone interface. Do not start a daemon, implement a phone app or add a second permanent terminal path in this plan.

## Progress


- [x] (2026-11-13) Read repository rules, program, architecture, protocol, core writers and Fable's review; confirm a clean `iphone-spike` at `f42d870`.
- [x] (2026-11-13) Preserve the accepted EP018 executable at `/tmp/carlos-ep018a/ep018-carlos` and save the independent review under `.agents/reports/exec018a-fable-review.md`.
- [x] (2026-11-13) Prove bounded output reads, compact change facts and optional watch DTOs. Nine core tests cover split controls/Unicode/diff prefixes, linear bytes, pause/resume, replacement, history adoption, reset/edit and retired ids; protocol fixtures include the new messages.
- [x] (2026-11-13) Inventory terminal workflows in `docs/terminal-client-migration.md`; revise architecture/program/protocol for headless-first EP019, typed native controls, terminal parity and separate deletion. Bindings/schema/coalescers wait for actual consumers/evidence.
- [x] (2026-11-13) Remove ClaudeDraftContext and callback-focus input. Return typed submitted-text outcomes with ClientOperation; carry origins across queued sends and helper switches. Backend read context now comes from session identity, never the selected panel. Add core and terminal two-client recovery tests.
- [x] (2026-11-13) Split send/steer, rewind/fork and rename into prepare/execute/finish. Work owns no session borrow, a single mutation slot preserves order, and native/session/turn guards reject stale completions. Six worker/channel tests prove concurrent reads/facts, busy rejection, reset/new-command fencing and early native consumption/completion. Remaining transport/lifecycle waits are explicit EP019 entry-point work.
- [x] (2026-11-13) Delete the 385-line source tokenizer/module walker and 18 synthetic source-tree tests; retain the all-features/all-platform package guard, add actual-workspace/metadata-argument checks and a Rust alias/inherent-impl compile-fail test. `scripts/test.sh` and CI retain the compiler and dependency gates.
- [x] (2026-11-13) Pass final Rust/feature/lint/release/compiler/package gates, 177 terminal and 23 native-monitor workflows, native Codex contracts and quiet latency/RSS checks. Direct engineering verdict PASS; every finding is fixed.
- [x] (2026-11-13) Record the final report/review and move this completed plan unchanged to `.agents/done/`. EP019 remains behind owner review; no install, merge or push.

## Surprises & Discoveries


EP018 emits tool text changes as facts and its output read returns the full body. This works for a collapsed row, but a remote terminal or phone showing a running tool would repeatedly transfer old text. Tool output can contain split ANSI escape sequences, partial lines and UTF-8 characters. A correct cursor must handle these without scanning the old body or copying a second full transcript. The terminal currently preserves ANSI colors, so changing its source text to a second canonical representation would risk fidelity.

The terminal still calls many core APIs directly. The public protocol covers basic phone commands, while native goals, queues, settings, tasks and rewind also need a deliberate migration route. Core BackendContext and ClaudeDraftContext borrow a client's selection, focus, composer and confirmation; ClaudeEffect includes editor instructions. A terminal-free crate alone does not remove this semantic dependency. Prompt methods also wait synchronously for backend acceptance, which cannot run inline on the future daemon's owner thread.

The output prototype keeps sparse replacement revisions and encodes a raw byte offset plus eight-state terminal parser, diff prefix and closed-body flag in each opaque token. Closed cursors explicitly restart on unexpected late appends. Retained history rows preserve revisions only when their actual body and kind match. A reset does not suppress revision changes on retained rows. The two-watcher test transfers 400 KiB once while each collapsed event stays below 400 bytes.

## Decision Log


Decision: keep all work in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`, without installing, merging or pushing. Execute directly, with no implementation subagent. Rationale: the owner chose this isolation and requested direct work; the per-EP review gate still applies. Date/Author: 2026-11-13, owner/Codex.

Decision: treat client protocol version 1 as provisional until the daemon and a real remote client exercise it. Rationale: there are no deployed clients to migrate, and freezing incomplete coverage now would add unnecessary compatibility code. Retain stable semantic contracts that are already useful. Date/Author: 2026-11-13, Codex.

Decision: use bounded output chunks with opaque, entry-bound cursors and per-entry body revisions. Separate authoritative row facts from optional output subscriptions, so collapsed clients receive no bulk output and session sequence numbers have no filtered gaps. Rationale: several watchers may consume different ranges; reconnects and body replacement must be explicit without storing another copy of every tool body. Prototype and record the exact cursor encoding before promoting it. Date/Author: 2026-11-13, Codex.

Decision: preserve existing terminal features and describe every capability before changing the TUI transport. Keep the shared Rust client objective; defer bindings, schema generation and timed coalescers until actual consumers or measurements need them. Rationale: the inventory should determine EP019's scope rather than force backend passthrough messages or silent feature loss. Date/Author: 2026-11-13, Codex.

Decision: record a recovered edited/queued prompt once as an unconditional session notice, without terminal key names. Keep F5 guidance and restore-versus-retain decisions in the terminal. Rationale: the prior core emitted a notice only when one terminal had newer typing, so two clients could not agree on shared facts. Recovery text and draft preservation remain intact; the notice is intentionally also visible when an editor was empty or identical. Updated wording-specific tests, without weakening text/ordering/backend-request assertions. Date/Author: 2026-11-13, Codex.

Decision: native callback acceptance reports the card, blocking fact and whether it became active. The terminal decides its focus and makes an explicit card-focus command; the receiver never inspects a focused-widget boolean. Native read completion uses the session's connection/epoch and original pending request target. The current bounded selected-task cache still serves the embedded terminal; EP019 must key requested task windows by target and aggregate connection read interests before cutover. Date/Author: 2026-11-13, Codex.

Decision: expose `prepare_turn_text`, `prepare_rewind_text`, `prepare_rename`, consuming `BackendWork::execute`, and `finish_backend_work`. Preparing/settling owns SessionState; executing owns immutable command data only. The terminal drives this sequence in process and the old blocking core entry points are deleted. One outstanding mutation per session returns Busy for another mutation, while reads and backend facts remain available. Reset/native identity changes retire the slot; old completions cannot clear a new slot. Rationale: preserve command order without a speculative async runtime or generic command framework. Date/Author: 2026-11-13, Codex.

Discovery: a native consumed/completed frame can now arrive before its send reply. Added guards so a late acknowledgement cannot duplicate the human row or resurrect a completed turn. EP019 must also separate native event normalization from generation-bound transport handles; the current BackendClient is not universally Sync (Claude keeps a RefCell handler and clients retain receiver fields). The worker proof moves prepared work, not a live shared native client. The headless EP019 gate must exercise real adapters before declaring the owner concurrent. Other approval/interrupt/settings/lifecycle I/O remains enumerated in the migration document; no daemon dispatcher calls it inline in this plan.

Decision: remove `scripts/check-session-boundary.py` and its source-string fixtures. Cargo/rustc enforce package dependencies, module compilation, foreign inherent impls and private fields. The remaining package guard traverses all declared feature/platform/dev/build edges by resolved package name; a new test verifies its Cargo arguments and another checks this real workspace. Review retains semantic no-editor/no-I/O rules that neither an identifier lexer nor the compiler can prove. Date/Author: 2026-11-13, Codex.

Discovery (2026-11-13): main advanced to `06f1e96137442aba5e7aea017fe40e5e8482f88c` with Escape-burst suppression and four native-Claude interrupt workflows. Rebased the 97-commit spike onto it, resolving terminal test-list/import overlaps and moving the new interrupted-at timestamp into ViewState. The interrupt wrapper keeps the suppression local after a real stop request. The main checkout remains untouched. Pre-rebase gates are retained as intermediate evidence; all final gates and executable measurements must run again on the rebased source.

Validation update (2026-11-13): final source `43b3460` passes 816 app + 250 core + 6 proto tests and three compiler checks; no-default passes 780 + 250 + 6 plus the same compiler checks. Strict Clippy and both releases pass. The release/contract runner passes 177 terminal workflows, 23 native-monitor workflows, isolated Codex 0.167.1 contracts, 59 SDK helper tests, package smoke and 11 architecture tests (`target/ui-artifacts/run-rpOhMe`). The accepted default executable is 20,188,600 bytes, SHA256 `3b7970cae7f4d33cc1687715ba6990e36315482324f8e015cc72fcff04c262be`.

Performance update: both quiet 31-round comparisons pass the original pre-plan reference and preserve typing/scroll p95. A repeat confirms about +0.05 ms full draw versus the old EP018 executable; other small positive intervals do not repeat consistently. Built rebased EP018 `d9ff901` in the separate `/tmp/carlos-ep018a-rebased-ep018` target to test the cleanup on a common main. Raw results are under `/tmp/carlos-ep018a/latency{,-repeat}`. Five-pair RSS sweeps give EP018/current medians 25,668/24,372 KiB at 1k turns, 32,192/31,060 at 2k and 45,712/44,744 at 4k. Anonymous RSS at 4k is 32,844 KiB for both; file-backed pages explain the total difference.

## Outcomes & Retrospective


EP018a is complete at runtime source `43b34609eee2912077b7fbecb70de5025773036a`. The core serves bounded running output, carries submitted-operation ownership without reading client editors, and prepares send/fork/name waits outside its state borrow. The terminal inventory makes native controls and real transport handles explicit EP019 requirements. The old lexical guard is gone; package and compiler checks remain.

All final gates pass. Two 31-round runs stay within the original pre-plan reference. A separate 61-round common-base control shows no positive 95% lower confidence bound on any metric; full draw remains +0.03 ms with [0.00,+0.10] uncertainty, so this report does not claim exact zero cost. Anonymous-memory medians match at 4k turns. The complete source/binary identities, raw evidence, remaining daemon work and accepted small binary-size increase are in `.agents/reports/exec018a-final-report.md`; the engineering review is `.agents/reports/exec018a-engineering-review.md`.

The proof found real defects worth addressing before sockets: reset/edit cursors, early consumed/completed replies, identical text from another client and queued prompts dequeued while a mutation waits. Their tests now pass. The actual daemon, generation-bound backend handles, remote command handlers, multiple task windows and terminal cutover remain EP019 work. The owner reviews this step before that work starts. Nothing was installed, merged or pushed.

## Context and Orientation


The workspace has the terminal binary, `crates/carlos-core` and `crates/carlos-proto`. AppState owns SessionState and ViewState. The core stores original source messages contiguously in `session/transcript.rs`; rows have opaque EntryIds. `session/transcript_edits.rs` records changes in a bounded journal. SessionState::publish returns Published, which borrows the session so callers finish encoding before another write. The terminal consumes the same records without serialization. `session/event_reads.rs` currently formats whole tool bodies for a remote caller.

A cursor is a small token describing how far a client has consumed one output body. A revision identifies that body's current contents after a non-append edit. The client can resume a cursor while the revision matches; a replacement requires a fresh read. These tokens are separate from the numbered session events that order conversation facts. An output subscription means the server sends chunks only for an expanded tool; per-client subscription state belongs to EP019's connection owner.

`session/backend_receive.rs` routes backend frames using BackendContext. Claude's receive modules return ClaudeUpdate with editor effects and use ClaudeDraftContext to decide draft recovery. Native panel controls also pass selected targets and visibility into handlers. The new boundary must accept explicit commands and correlation data, never ask which client's current editor is empty. A client message id identifies one submitted operation so only that client handles rejection or recovery. Existing native backend ids remain internal.

## Plan of Work


### Milestone 1: incremental output that a client can resume


Add protocol types and core reads for bounded chunks, with revisions and opaque cursors bound to an entry. Keep raw source text as the one stored body so terminal colors and copy behavior stay intact. A chunk encoder must consume each new source byte once, retain only bounded parser state in its cursor, strip terminal controls across chunk boundaries, and retain semantic line kinds and newlines. It must not reparse a growing prefix to resume. Track body revisions only where necessary, preserving them correctly through history adoption and retiring them with rows.

Add output-change records that do not ship bulk text to collapsed clients. Specify subscribe/unsubscribe and resume behavior in the provisional protocol; provide pure core reads and a headless two-client demonstration rather than implementing sockets. One watcher opens a running tool, pauses, resumes and survives a body reset while the other stays collapsed. Assert exact Unicode text, diff classifications, no leaked ANSI controls, bounded frames, malformed/stale cursor errors and transferred bytes proportional to the output. Measure the prototype before committing to its representation; reject a second full-body cache or growing-prefix scan.

### Milestone 2: price the terminal migration


Inspect direct session calls and group every public workflow into local view work, shared facts/queries, session commands and backend I/O. Write `docs/terminal-client-migration.md` using lists with source locations and the intended typed command/result route. Include native panels, callbacks, questions, goals, queues, settings, tasks, naming, prompt edit/fork, quota, Ralph and dictation. State which wire messages already cover each workflow, which additions EP019 must implement, and which operations stay local. Do not make opaque backend JSON passthrough the permanent answer.

Update `docs/engine-architecture.md`, `PROGRAM_PLAN.md` and `docs/client-protocol-v1.md` to reflect this pre-daemon step and the revised EP019 milestones. The first daemon test must demonstrate commands and live facts through a headless client. The TUI cutover must prove parity against the inventory before deleting the embedded path. Client library bindings and schema output must have a concrete consumer before adding dependencies or build machinery.

### Milestone 3: let each client own its drafts


Replace core composer comparisons and editor instructions with facts about submitted commands, recovered submitted text, retired cards and accepted conversation changes. Carry an opaque client operation id where an asynchronous result needs its originating client. Move decisions about newer typing, restoring/replacing a draft, confirmation, focus, scroll and retained-draft lists into terminal view code. Preserve the exact current behavior with the existing recovery/parity tests, and add a two-client test proving a rejection cannot overwrite another client's draft.

Remove ClaudeDraftContext and the composer/confirmation members of backend input. Replace ClaudeEffect's editor verbs with semantic outcomes, rather than renaming the same editor protocol. Explicit target/read-interest data for native controls must be independent of a single global panel or focused callback; the inventory should distinguish real backend request scope from view state. Keep authoritative state updates in the core and targeted view reactions in each client. This milestone ends only when backend frame handling needs no live editor to proceed.

### Milestone 4: separate backend waits from state changes


Expose prepared backend work and completion handling through a narrow command boundary. Preparing and settling an operation may mutate the owned session; executing a backend call must not borrow it. The existing terminal can orchestrate these steps in process while EP019 later schedules the same work outside its owner loop. Preserve command correlation, native connection/turn guards, request ordering, timeouts and failed/uncertain send behavior. Do not add a second parallel command implementation or an async runtime.

Prove the boundary with a delayed deterministic backend: while one request waits, the owner can process a backend fact or another client's read, and the eventual completion affects only the matching live operation. Enumerate remaining transport-only waits in the migration document so EP019 cannot accidentally call an old blocking convenience method in its owner loop.

### Milestone 5: remove redundant guards and close the step


Audit `scripts/check-session-boundary.py` against the compiler and `scripts/check-core-dependencies.py`. Remove the hand-written Rust tokenizer/module walker and pre-workspace branches once their required checks have a concrete remaining home. Retain the all-features/all-platform dependency checks, private session fields and useful negative compiler tests. Update scripts, CI and architecture tests together; do not weaken the terminal-free core invariant merely to reduce lines.

Run the engineering reviewer prompt directly, label the self-review honestly, fix blocking findings and keep the plan current. Run the full checks and measure the actual executable. Record exact source/compiler/binary identity and raw evidence. Close only after the terminal still passes its deterministic workflows and speed meets the accepted baseline.

## Concrete Steps


Run every shell command through `rtk`, from `/var/home/wegel/work/perso/carlos-daemon`. Begin with focused protocol/core tests while prototyping output reads, then run `rtk proxy cargo test` after each meaningful change. Before closeout run `rtk proxy cargo test --no-default-features`, strict Clippy with and without default features, and both release builds. Restore the default executable before `rtk proxy bash scripts/test.sh --release --codex-contract`. Do not replace a release binary while terminal checks use it.

Use `carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48` and `carlos perf-session src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48` with preserved EP018 and original pre-plan binaries. Save new raw results under `/tmp/carlos-ep018a/`. Run paired, CPU-pinned samples without competing builds or UI tests. Measure peak RSS through Linux wait4; the older built-in zero fields remain unusable. Never share Cargo target directories across different source checkouts.

## Validation and Acceptance


A headless client must read a running tool in small chunks, continue at its cursor, and receive exactly the same final text and line kinds as one full read. Streams split inside UTF-8-safe source fragments, ANSI sequences, diff prefixes and unterminated final lines must remain correct. Large output and long single lines must not turn many reads into growing-prefix work. Replacement must invalidate the old revision; a label or status change must not invalidate unchanged output. Collapsed clients receive only compact facts. Reconnects, overflow resets, history adoption and retired ids must produce deterministic recovery.

A backend event must be handled without any client's current composer or focus. Existing terminal tests must retain typed-ahead drafts, confirmation, failed-edit recovery, queued-human priority and exact backend requests. Two clients with different drafts must handle the same session facts independently; only the initiating client receives its submitted-operation recovery. A prepared backend wait must leave the session available for other events until the matching completion arrives.

The migration document must account for the terminal's existing capabilities and define a typed route for each shared operation. The reduced guards must still reject terminal/audio dependencies in the core and I/O dependencies in the protocol crate. All meaningful Rust, terminal, contract, feature and lint checks pass. Changed Rust files and functions stay within 400/60 lines. Measurements must show no repeatable latency regression; document fixed memory costs honestly.

## Idempotence and Recovery


The worktree started clean at `f42d870`. Keep small buildable commits at output, inventory, draft-boundary, backend-work, guard and closeout milestones. Focused prototype changes may remain additive until proved; remove the obsolete path in a separate commit before closing. Never reset the owner's main checkout or rewrite completed EPs. No live user session, installed binary or remote branch is part of this work. Stop at the per-EP review gate after EP018a; do not begin EP019 automatically.

## Artifacts and Notes


EP018's preserved release SHA256 is `ae5b517c49bba678c6d6b9f6513ce2e3bd4e3200daaec22cff203747e3c94ca8`. Its final report is `.agents/reports/exec018-final-report.md`. The original latency reference is `/tmp/carlos-base/target/release/carlos`, hash `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`. Preserve the insertion inline hints measured in EP017. Fable's read-only review at `f42d870` is copied into `.agents/reports/exec018a-fable-review.md`; the model metadata recorded claude-fable-5-2, medium.

## Interfaces and Dependencies


Keep `carlos-proto` pure serde data, `carlos-core` terminal-free, and the terminal responsible for view state. Use existing standard threads/channels and dependencies. Do not introduce Tokio, a generic event framework, speculative bindings or a second transcript copy. Final core interfaces must include a bounded output-chunk read on Published, opaque output cursor/revision types, explicit body-change facts and prepared/settled session commands whose backend execution owns no session borrow. The promoted read is `Published::output_chunk(entry, cursor, limit) -> Result<OutputChunk, OutputReadError>`. It consumes at most 64 KiB source and emits at most 256 segments. The token currently encodes `o1:entry:revision:offset:control:line:prefix:closed`; its syntax remains opaque to clients. `OutputChanged` stays in the ordinary numbered session stream, while optional `OutputUpdate` carries its own resume cursor and stamps (never advances) the session sequence. No watcher scheduler or socket is implemented here.

## Engineering Review


Direct review against `.agents/reviewers/engineering_reviewer.md`, initial scope `f42d870..a66f61f`: submitted-output/command boundary, tests, source constraints and roadmap. No independent reviewer is claimed. The supplied Fable review is independent evidence about the starting architecture only.

Initial findings: MAJOR: edit dispatch/bind/failure still matched human text without its ClientOperation, allowing another client's identical text to claim an edit. MAJOR: failed automatic work could return an unowned editor recovery, and an unrelated acknowledgement could clear a failed-edit badge. MINOR: output controls handled C1 CSI but not C1 OSC/DCS/ST or intermediate ESC sequences. Corrective guidance: match origins as well as text, recover only human submissions, emit edit acceptance only for its exact UUID, and extend the bounded parser with explicit cases and tests. Initial verdict: FAIL, resolved by the fixes and final gates recorded below. A final queue audit also found that take_dispatchable_prompt could dequeue text while a prepared mutation held the slot; it now retains the queue until completion, with the delayed-worker test checking the exact human text afterward.

Update note (2026-11-13): created EP018a before source changes after the owner approved the broader pre-daemon cleanup. Recorded live output, client independence, terminal coverage, backend waits and guard removal as concrete acceptance criteria.

Final engineering verdict (2026-11-13): PASS. `.agents/reports/exec018a-engineering-review.md` records the resolved findings, corrective guidance and proof. The final release/contract gate and common-base/original-reference speed gates pass. No independent reviewer is claimed. Closed for owner review before EP019.

Update note (2026-11-13 closeout): all milestones and evidence are complete; corrected the removed guard count to 385 lines/18 fixture tests, documented the rebase and common-base control, and recorded the remaining EP019 transport/command work without claiming a daemon exists.
