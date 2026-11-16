# EP021a: share checked panel readers before adding more clients

This living ExecPlan follows .agents/PLANS.md. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. Work directly in /var/home/wegel/work/perso/carlos-daemon on iphone-spike. EP020 is complete and approved under the owner's delegated grant. Do not edit its archived plan, replace the installed executable, modify the owner's frozen preview, merge main or push. The owner asked Root to work directly; do not spawn developer workers. At the end, pass the complete checks, perform the direct engineering review and obtain Fable 5.2 medium's whole-roadmap review before delegated approval. Then write and continue EP021b; do not invent a human gate.

## Purpose / Big Picture


A phone or web UI should be able to open settings, a goal, the native prompt queue and native task panels by passing messages through carlos-client once. Today the terminal contains the rules that join pages, wait for matching numbered facts, reject stale replies and fetch long fields. Another client would have to copy those rules. Move that work into the shared Rust client and delete the terminal copies, while keeping the terminal's keys, layout, selected row and editor drafts local.

The terminal must keep the same visible behavior. A small test-side Rust consumer must also read complete supported panels through an actual daemon without importing terminal controllers. This is the observable proof that the phone can reuse the work. The native backends and wire protocol remain the same unless a concrete correctness issue requires a small documented change.

EP021 is split at a natural boundary. This plan shares existing read behavior and completes the small EP020 follow-ups. EP021b will add paged native history, Grok approvals and richer tool data after these common readers exist. EP022 still builds UniFFI bindings and the actual Swift iPhone app, then measures that consumer on the remote Mac. Do not add speculative phone commands, a generic panel framework or a second native owner here.

## Progress

- [x] (2026-11-16) Add one exhaustive command_info.rs description used by ingress, manager routing and every mutation retry-class choice, including asynchronous open. Remove wildcard read fallback and repeated Native/Core class literals; name Prepared fields instead of tuple positions. The intermediate49 daemon Rust cases and full1657 default workspace cases pass, strict default lint and400/60 audit pass, and matching debug executables build. The complete186 actual-daemon regression passes106.507s at /tmp/carlos-ep021a-admission.log. Both feature/final release/performance checks remain required at this plan's end.


- [x] (2026-11-16) Read the completed EP020 gates, exact-pair measurements, both final Fable reports and all decisions. Identify repeated queue/settings/goal/task read settlement in the terminal and checked low-level reads already present in carlos-client.
- [x] (2026-11-16) Split shared-reader/admission cleanup into EP021a and leave native feature work in EP021b, whose full plan will be written after this one ends.
- [x] (2026-11-16) Rename Stage::Written/printed written to Returned/returned in both native writers and real-pipe assertions. Add a signalled real reader that drains256KiB for more than500ms, pauses with its4MiB frame still charged, then observes automatic peer closure and zero charge without external Drop/close. The focused test passes1.86s; all1657 default workspace cases pass18.96s and the400/60 audit passes. The healthy paced and never-reading cases remain. Full final gate/performance checks stay required after the cleanup.
- [x] (2026-11-16) Replace hand-kept daemon admission groups with one exhaustive command description used by ingress, typed routing and all mutation retry classes. The complete186 actual-daemon cases pass106.507s at /tmp/carlos-ep021a-admission.log, including full-queue stop/control/operator/local reserves and native/private recovery. Default workspace1657, strict lint, debug pair build and400/60 source audit pass. Preserve FIFO, budgets, scopes and canonical wire bytes.
- [ ] Prove one shared queue reader in carlos-client and remove its matching terminal page/reply driver.
- [ ] Move settings, goal and task list/detail/window/activity/overview readers into typed shared client models, preserving all current bounds and stale/root/reader checks; remove matching terminal copies.
- [ ] Examine duplicate wire/Engine command payload wrappers and replace clear identical records with named payloads, preserving distinct scopes and flat wire shapes. Record any justified remaining wrappers.
- [ ] Prove actual-daemon headless shared-panel use, both feature/lint matrices, complete release/terminal/native checks and matched performance/source/count evidence.
- [ ] Complete direct engineering and Fable whole-roadmap reviews, record dispositions and delegated approval, archive unchanged, then write EP021b and continue.

## Surprises & Discoveries

The command description needs distinct handler, ingress and retry fields: device/list uses the operator ingress reserve without a retained mutation result; stop uses lifecycle reserve; reopen is ordinary; asynchronous native open must retain the same ordinary class after its checked target completes. A single role cannot replace these independent rules. Named Prepared fields also remove a tuple-position dependency when attaching a captured task window. No command method names, wire payloads or canonical bytes changed.

A test cannot equate the final application read with the exact final TCP write, because both kernel buffers can still move data afterward. The new test synchronizes the application pause, proves a charged frame remains after more than500ms of real draining, then waits for the worker's own timeout/teardown within a broad bounded allowance. It asserts automatic close and zero charge, not an invented exact500ms delay from the pause signal. A paused client stays open until the server closes it. Capture outbox counts outside the mutex guard before assertions so a failing assertion does not poison the worker's teardown lock.


The client already parses and validates every low-level panel reply, but Effects.replies still hands those replies to platform code. For example src/app/remote/queue_reads.rs holds one pending request and one future page, calls Client.merge_queue only when the matching numbered facts arrive, and starts each following page/text read. Settings and tasks repeat the pattern. Sharing the decoder alone has not shared the reader.

Current queue/task page windows retain only one bounded preview page and opaque page cursors. Queue covers its existing 128-page allowance; task lists cover the existing 4,352-row ceiling. Task details allow eight MiB. The terminal's selection code includes deferred arrow movements before a refreshed task list settles. Preserve those behaviors while changing who owns the read state; do not replace them with a truncated first page or silently select a different native task.

EP020's focused Fable follow-up approves closure with three MINORs. A reader that first makes progress and then stalls still needs an explicit socket test. The timing field currently called written records an I/O attempt returning even on error, so its label should say returned. The real-pipe/Python and paced-reader timing tests have wide margins; change them only if a reproducible failure justifies it. The old unexplained 500 ms Claude event remains a recorded risk, not a root-cause repair.

## Decision Log

- Decision: keep one small CommandInfo record with handler, ingress and optional retry class, selected by an exhaustive match on all Command variants. Group a few related variants per arm to keep the function readable within60 lines. Existing handlers retain their scopes and static budgets. Asynchronous open uses the same OPEN description rather than a second class literal. Rationale: new commands require an explicit choice without a registry, generic handler framework or broad wire API change. Date/author: 2026-11-16, Root.

- Decision: name the native diagnostic stage Returned and describe an attempted write/flush returning even on error. Keep all timestamps and pre-cleanup snapshots. Rationale: a write returning is not proof of success or native acceptance. Date/author: 2026-11-16, Root.
- Decision: test partial progress with a paced real TCP stream and explicit pause/release channels, keeping the socket open and frame larger than its kernel buffers. Rationale: it exercises the new deadline state and verifies actual worker cleanup without adding a production observer, generic I/O framework or minimum transfer rate. Date/author: 2026-11-16, Root.


- Decision: share read state inside the existing pure Client, with typed panel models and Effects, rather than add another platform wrapper that each language must drive. Rationale: the platform should feed a message once and obtain changed models plus outgoing requests. Date/author: 2026-11-16, Root.
- Decision: keep keyboard handling, focus, scroll, rendering, native local preferences, TextArea editors, private save intents and UI recovery text in the terminal. Rationale: those fields do not describe server facts or common page reads and would pull terminal/Core dependencies into the pure client. Date/author: 2026-11-16, Root.
- Decision: preserve complete supported lists, captured older native windows and per-reader/root guards. Rationale: a capped snapshot is not behaviorally equivalent. Date/author: 2026-11-16, Root.
- Decision: use one exhaustive command description rather than a wildcard that treats a future mutation as a read. Keep handler role, ingress allowance and retry class distinct fields of that description. Rationale: these are different rules, but one command must receive them in one checked place. Date/author: 2026-11-16, Root.
- Decision: prototype the queue family first, then use the demonstrated pattern for the other families. Rationale: it exercises cursor walks, long text, future replies and editor independence without inventing abstractions for every panel before one works. Date/author: 2026-11-16, Root.

## Outcomes & Retrospective


The first two EP020 follow-ups are implemented and the default workspace tests pass: native timeout text now says returned, and a real progressing-then-stalled reader closes automatically and releases its active charge. The accepted EP020 pair remains unchanged as the baseline. The exhaustive description now drives ingress, typed handlers and mutation retry classes; unit/default/lint checks and all186 actual-daemon cases pass. The first shared queue reader remains next after that proof. This plan must end with one reusable set of read controllers and fewer matching terminal controllers, not a parallel implementation kept indefinitely.

## Context and Orientation


The daemon owns one mutable native session in carlos-core and accepts Unix or authenticated WebSocket clients through one manager. carlos-proto holds the typed wire data. carlos-client is a pure state machine: platforms own sockets, clocks and rendering and pass each incoming message to Client. It returns Effects with outgoing requests and changed session/body/operation data. Its Replica is the one accepted copy of a session's numbered facts; a read cannot replace it with stale or future facts.

crates/carlos-client/src/client.rs defines Client, Effects, ReadReply, RequestFailure and per-session Slot. client_replies.rs, pending.rs and client_requests.rs correlate reads. settings_reads.rs, task_state.rs, task_overview.rs and sibling modules already validate typed pages, revisions, byte ranges and session ownership. client_budget.rs enforces the combined 64 MiB fact allowance; body/operation/catalog caches have their own existing bounds. goal_watches.rs and task_watches.rs restore wanted readers only after a current subscription succeeds. Extend these existing paths rather than create another connection or request tracker.

The terminal's Device owns a Client plus UI state. src/app/remote/queue_reads.rs and queue_pages.rs contain the queue read controller; queue_editor.rs, queue_outcomes.rs and queue.rs hold editor/private operation and key state. settings.rs mixes Catalog/Fetch with UI defaults and save intents; extract only the former. goals.rs repeats reply settlement around the already shared goal facts. tasks_incoming.rs, tasks_reads.rs, tasks_pages.rs, tasks_details.rs, tasks_text.rs, tasks_windows.rs, tasks_activity_reads.rs and tasks_overview.rs hold read/page/field rules. tasks_move.rs has the checked deferred relative movement behavior. tasks_render.rs, tasks_activity_render.rs and paint caches stay terminal-specific.

crates/carlosd/src/transport/ingress.rs assigns count/byte allowances with a wildcard read default. manager/commands.rs repeats handler groups and manager/mutation_work.rs plus its helpers return a retry Class. Create one focused daemon command-description module and remove those independent classifications. Preserve FIFO, separate local/network allowances, operator/lifecycle reserves and native control priority.

crates/carlos-core/src/write_timing.rs and the Claude/Grok writers hold EP020's numeric diagnostics. crates/carlosd/src/network/handshake.rs counts real writes; transport/websocket.rs applies the no-progress deadline. transport/websocket_write_tests.rs has a paced real socket fixture and the blocked charged-frame test. tests/daemon/test_network_source.py proves actual stalled-peer slot release and independent native controls.

The normal scripts/test.sh runs real daemon, terminal and installed native contracts without model inference. tests/daemon/support.py provides private homes and native fixtures; tests/ui/terminal_daemon.py attaches actual terminals. crates/carlosd/tests/client_wire.rs already exercises the pure Client over a real Unix socket and is a useful starting point for a test-side panel consumer. Do not add I/O dependencies or a production fixture-injection command to carlos-client.

## Plan of Work


### Milestone 1: finish the precise socket and diagnostic follow-ups


Rename Stage::Written and its printed label to Returned in write_timing.rs and both writers/tests; describe a write/flush attempt returning, not successful acceptance. Keep all timestamps, redaction, the pre-cleanup Grok snapshot and both native 500 ms deadlines. No change authorizes resending an uncertain command.

Extend the real paced socket fixture to drain several chunks of a multi-chunk frame and then stop. Keep the client socket open and its unread tail large enough to block the server. Prove that the peer stays connected while progressing, closes after the last real write progress stops, and releases the active outbox charge. Synchronize the drain/stop boundary with a test-owned signal rather than guessing which read happened. Use a bounded overall fixture deadline and retain the original healthy reader and never-reading cases. No timer, rate cap or larger production timeout is needed. Run the focused tests and cargo test before committing.

### Milestone 2: describe every command once


Add crates/carlosd/src/command_info.rs with a small internal record describing the handler role, ingress class and optional mutation retry class for a borrowed Command. Its authoritative match must name every Command variant with no wildcard. Keep it data-only: no transport trait, registration system or handler closure. Ingress derives its allowance from this record; Manager chooses its existing typed handler role from it; mutation admission obtains the same retry class rather than returning another hand-kept class from payload preparation. Keep the different limits and handler roles distinct.

Preserve exact existing classes, including stop's independent lifecycle reserve, reopen/create ordinary work, operator device methods and urgent interrupt/approval/task-stop controls. Hello is still authenticated before routing, and a network peer never gains local operator rights. Add meaningful tests for full-queue controls, operator/local reserves and representative reads/mutations. Compilation must require a new command to receive a description. Keep all existing actual-daemon fault workflows green.

### Milestone 3: make one queue reader reusable


Introduce a focused panels module in carlos-client and a typed QueuePanel model. The Client owns pending read ids, cursor/page windows, future-page settlement, text reads and revision-scoped read errors. It consumes only replies belonging to its own panel controller; direct application reads that still use Effects.replies remain usable. Client.receive/drive integrates the controller with existing Replica changes, request tracking and outgoing Effects. A future reply waits for the numbered facts without a general unbounded event queue; an old reply or failure cannot replace newer data.

Expose a small typed API, beginning with PanelKind, Client.open_panel(session, kind), Client.close_panel(session, kind), a typed queue model accessor and explicit queue position/entry targeting. Effects must identify the session/panel that changed. The first queue proof decides the exact additional target accessor names and records them here before copying the pattern. This is an ordinary implementation choice, not a human gate. Do not add a generic controller trait, mutable Replica escape hatch or native JSON method.

Port the existing page/cursor rules before simplifying them. Keep the queue's complete supported ordinal range, selected-entry anchoring, one-page preview retention, UTF-8 full-text reads and pending/failed state. The platform supplies the desired target and keeps its visual selection/editor. Shared code may resolve a requested opaque target to a proven row; it must never authorize a different selected mutation after a revision changes. Preserve explicit refresh, disconnected reads and no automatic mutation replay.

Change the terminal to open/close/target and render that shared model. Keep TextArea, local queue mode, keyboard events, editor versions, private submitted edits and recovery notices in the terminal. Delete the corresponding terminal read/page settlement code in the same buildable sequence; do not leave the old controller as a fallback. Add pure-client tests for future-before-fact, stale failure, reconnect, close while offline, complete cursor walks and full text. Run the actual queue terminal workflows and prove two devices retain different drafts and outcomes.

### Milestone 4: share the remaining supported panel readers


Move Catalog/Fetch out of terminal settings.rs into a typed shared settings model. Keep RuntimeDefaults, save intents and UI settings conversion in the terminal because carlos-client may depend only on protocol/data packages. Move goal read settlement around the existing single goal facts, without adding a second goal copy or client polling clock.

Then share task headers, current pages, captured older windows, selected details and field chunks, child activity and overview reads. Preserve original request ids, sequence/revision/root/window/run fences, explicit reader close, reconnect restore order, failed-read state and bounded retired tokens. Use the existing pure validators rather than revalidate by unrelated string checks. Preserve complete supported lists, long labels and 4,352-row/current native limits; no new list/body cap or truncated first-page substitute. Maintain deferred movement semantics when refreshed counts arrive after arrow bursts.

Retain existing per-family byte/count bounds and include shared panel models in a checked combined device budget, using the existing 64 MiB fact allowance rather than an unbounded map per session. On capacity failure, retire the affected reader/model explicitly and preserve private submitted text and backend work; do not silently truncate facts, evict another panel's selected text or create an automatic refetch loop. Closing a panel or losing its session must retire its outstanding reads and server interest exactly once. No timers, sockets, threads, terminal or Core backend dependency may enter carlos-client.

Remove matching terminal read/page/field controllers as each family migrates. Keep renderers, key bindings, focus/scroll, private editors/save intents, platform preferences and paint caches. Report moved/deleted production code separately from tests; no arbitrary line-count quota replaces behavior. The end must contain one set of common readers, not two synchronized copies.

### Milestone 5: remove only clear command-record repetition


Examine the wire Command to EngineCommand translators alongside the shared-reader changes. Reuse named payload records where their fields and meaning are identical. Keep public session addresses separate from owner-local work and retain private dispatch guards. Preserve flat v1 wire envelopes and canonical retry bytes unless a tested correction requires a documented provisional change. Do not create a generic command builder or reuse a wire record that weakens the captured owner's scope. Record concrete retained wrappers and their reason in the final review decisions; do not rewrite every command for cosmetic symmetry.

### Milestone 6: prove reuse, parity, speed and the whole roadmap


Add a narrow test-side Rust consumer under the daemon crate that uses the public Client panel API against the existing actual native/daemon fixture. It must traverse more than one page, read a long field, close/reopen and reconnect, and reject an old root/window reply without importing terminal controllers. Reuse the production socket/client codecs. Keep all fixture I/O in the daemon/test side; do not add I/O dependencies or synthetic injection to the pure client. Assert facts and requests, not just a successful decode.

Run both Rust configurations and strict lint, architecture/dependency checks and the complete stock release gate with Codex/Claude contracts. Add terminal screen/request regressions for changed workflows; preserve their original deadlines. The root shell needs a private script copy that changes only cwd/artifact root to /tmp, as EP020 did. Preserve failed traces and investigate the first failure before rerunning. A native 500 ms recurrence must retain the new stage evidence rather than receive a longer deadline.

After every build/test finishes, freeze the matched binary/helper pair and compare with the accepted EP020 corrected-pair. Run 15 alternating renderer samples on CPU 6 using the same synthetic 1000 turns/seed 1/24 tool lines/160x48 and captured Core fixture. Repeat the actual 1000-native-prompt Unix/WebSocket admission and exact-source comparisons; report scope, p95/max, fixed RSS and file-size costs. Add a bounded large-panel read measurement so sharing code does not make every keystroke clone or rescan all loaded rows. Keep source hashes/counts and extreme dense-Unicode limits visible. Do not benchmark during tests, compiles, hooks or review workers.

At the end, perform the direct engineering review and one Fable 5.2 medium review of the whole phone/web objective, the cleanup and next native/iOS steps. Address concrete findings and record every disposition. Only after all checks and reviews pass, record delegated approval, finish the four living sections, move this plan unchanged to .agents/done/, update PROGRAM_PLAN.md and write EP021b in full. Continue without the former per-EP human gate.

## Concrete Steps


Run all shell commands through rtk from /var/home/wegel/work/perso/carlos-daemon. Search by intent with semeja and use exhaustive literal searches where needed. Keep each commit buildable and use scoped imperative subjects.

    rtk cargo test --workspace --all-targets
    rtk cargo test --workspace --all-targets --no-default-features
    rtk cargo clippy --workspace --all-targets -- -D warnings
    rtk cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract

In this root shell, run the final command through the documented private artifact-root adaptation, keeping the stock tests/deadlines intact. Before the first code edit, read the relevant existing read controller and pure validator. The first focused checks should cover write_timing and the actual WebSocket writer, then daemon command classes and the first shared queue reader. Record exact test names and output paths as they are added.

## Validation and Acceptance


A terminal user opens settings, goal, queue, task lists, captured older tasks, details and activity, reads complete supported pages/long text, closes/reopens and reconnects, with the same visible rows and backend requests. Different devices keep separate drafts, selected rows and private results. A stale/failed/foreign/future read never selects a new native action or overwrites newer accepted facts. Closing while offline cannot restore an unwanted native reader later.

A second Rust consumer drives the same Client API without terminal controllers and obtains the same checked data. It feeds each incoming message once and receives typed changed-panel data and outgoing reads; it does not implement its own cursor walk, future-reply queue or root validation. The terminal no longer contains those duplicated controllers. New commands require an explicit admission description. A progressing socket remains connected, a later stall releases its charge at the unchanged stall rule, and native timeout text names write-return honestly without exposing private text.

All final checks pass on the exact frozen source. Interactive and panel-read measurements preserve speed and bounded memory, with any measured fixed cost explained. No body cap, native JSON bypass, new polling clock, async framework or permanent parallel controller path is acceptable.

## Idempotence and Recovery


Use fresh private fixture directories and exact test-owned PIDs. Leave the owner's long-running Carlos/Codex processes and frozen preview alone. Do not remove earlier failed artifacts, overwrite the accepted EP020 pair or install a spike binary. Common read controllers must be removable per session/panel and retire their old request ids, readers and bounded caches; disconnect is not permission to replay a mutation. Keep small commits so a controller migration can be checked independently while the tree remains usable.

## Artifacts and Notes


Accepted EP020 runtime source: 6a31fd059ad6f8f0e7e92b1f001bc765. Runtime corrections: 3c83655 and 272fae3; frozen pair HEAD 03658363d88f7f00abedfa3156be2fe578b84040. Baseline identities: /tmp/carlos-ep020/corrected-pair/identity.json. Final reports: .agents/reports/exec020-final-report.md and exec020-final-measurements.json. The final complete gate is network-release-gate-04.log, artifacts lifecycle-gate-5v6UpS. Main remains a29b1a1e3720e48b09e2ca984fb16c29b5857ea3; installed Carlos SHA256 remains d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3.

Baseline synthetic renderer scroll p95 is 0.68 ms, typing p95 0.48 ms, cold replay median 49.88 ms and peak RSS median 25,916 KiB. Actual admission medians are about 0.561 ms Unix and 0.700 ms WebSocket, with exact 10,092,544-byte source transfers in 155 chunks. These are matched-run reference figures, not substitutes for fresh alternating measurements. Dense Unicode terminal behavior still has its separately documented about six-second/1.3 GiB temporary-memory limit; do not hide it or cap bodies to improve a headline.

## Interfaces and Dependencies


Keep carlos-client free of I/O, wall-clock reads, threads, terminal, audio, speech and native backend packages. Use existing carlos-proto data and typed ids. Shared panel models are device-owned read caches around the Client's single accepted Replica, not a second session authority. Start with PanelKind, typed queue access, open/close/target methods and Effects changes; record the proven API after the queue milestone, then extend it specifically for other supported families. Keep unowned explicit reads usable through the existing reply interface.

The daemon command description must remain one exhaustive, data-only mapping to existing handler roles and budgets. Do not add a generic transport/handler framework. Named command payloads must preserve captured-owner scope and checked retry identity. Native feature work belongs to EP021b and iOS/UniFFI work to EP022; only those later consumers justify their bindings or additional dependencies.

Revision 2026-11-16: create EP021a after delegated EP020 approval, splitting required shared-reader/admission cleanup from later native features. Include all accepted whole-roadmap findings and the non-blocking focused follow-up tests/diagnostic label change without reopening EP020's checked runtime.

Revision 2026-11-16: finish the two focused EP020 follow-ups, record the real partial-progress proof and truthful write-return diagnostics, and keep exhaustive admission/shared queue reads next. No reviewer runs before this whole plan is ready.

Revision 2026-11-16: connect exhaustive command descriptions to actual ingress/routing/retry admission, retain the existing reserves/scopes and record the green Rust/lint/build checks while the full daemon regression runs. No independent reviewer has started in EP021a.

Revision 2026-11-16: complete the unified admission slice after all186 actual-daemon cases and default/lint/source checks pass. The shared queue reader is the next incomplete milestone; the full final feature/release/performance/review gates remain required after all reader migrations.
