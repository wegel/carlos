# EP021b: provide native history, approvals and tool facts for the first phone screens

This living ExecPlan follows .agents/PLANS.md. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current. Work directly in /var/home/wegel/work/perso/carlos-daemon on iphone-spike; do not spawn developer workers. EP021a is approved and archived. The owner delegated per-EP approval after complete checks, direct engineering review and one end-only Fable5.2 medium whole-roadmap review. Continue without routine human confirmation. Do not install, merge, push, modify main, replace either frozen owner preview or edit archived plans.

## Purpose / Big Picture


A phone should display the engine's tool kind, outcome, short result and diff counts without parsing terminal text. A user should answer a real Grok permission request through the same shared approval cards as Codex and Claude; the transport must not silently choose Allow. When a backend offers native history pages, opening a large session should fetch its newest complete window first and fetch earlier windows only when requested. Every supported older entry must remain accessible with its original native identity and original source.

The terminal is the real consumer during this plan. The actual first iPhone app and UniFFI bindings remain EP022. tmp/main-screen.md sections6.5,6.6 and11 specify the phone needs; EP018a supersedes that file's original ban on expanding running tools. Export only data those screens need. Do not add queue/goal/task screens or Swift mirror records for the Rust-only readers completed in EP021a. A later web client reuses the same checked facts, bodies, approvals and history.

## Progress


- [x] (2026-11-16) Read the accepted EP021a reports, final review decisions, roadmap, original phone engine requirements and current producers. Establish baseline source896435d22d03d85dba4cef0f58ba68e8 at checked tree3ffd3eb; closure commit4407f6e changes documents only.
- [x] (2026-11-16) Confirm that protocol_history.rs already calls public native turn/item pages but drains them all before display; sessions.read already exposes bounded Claude archive pages; Grok currently forces yoloMode and directly auto-allows native permission requests. Confirm shared Tool already has kind/status/summary/added/removed, but event_rows::export_tool emits summary/added/removed asNone and often kindOther.
- [x] (2026-11-16) Add the per-feature source ledger and an actual-daemon tool-fact regression. Against the frozen accepted daemon it fails in0.350s for the intended missing Command kind; trace /tmp/carlos-ep021b/tool-facts-before.log. Native history/Grok capability checks remain with their milestones before those producers change.
- [ ] Populate accurate shared tool facts and short summaries without adding a UI parser or changing original source/color/copy behavior.
  - [x] (2026-11-16) Cache native facts on each transcript entry, separately from its legacy label. Preserve explicit success/failure/interruption and unknown completion; export result line counts/diff counts through existing Tool fields. Four native-event tests and the full Rust workspace pass in /tmp/carlos-ep021b/tool-facts-rust-04.log. The original actual-daemon regression passes in0.32s with exact existing command/source bytes, /tmp/carlos-ep021b/tool-facts-after.log.
  - [x] (2026-11-16) Validate the first buildable cache/export piece: daemon188 tests in105.729s (one optional consumer skipped), selected actual terminal tool/color/summary20 tests in18.655s, strict workspace lint, fmt, diff and400/60 shape checks pass. Logs tool-facts-daemon-01.log, tool-facts-ui-01.log and tool-facts-lint-01.log under /tmp/carlos-ep021b. Production adds219/deletes50 nonblank lines, net169, allCore; count report exec021b-tool-facts-source-count.json. No install/preview replacement.
  - [ ] Finish backend metadata preservation and remaining negative/history cases, then run the full required script gate before this milestone closes.
- [ ] Replace Grok's automatic permission reply with scoped shared approval cards and exact native answers, with actual process/daemon/terminal checks.
- [ ] Prove and implement newest-window-first native history where supported, with complete older traversal, checked native scope, responsive controls and honest compatibility behavior.
- [ ] Pass both Rust configurations/lints, architecture/dependency/400-line/60-line checks, the complete corrected release/native gate and exact-pair measurements. Record production source added/deleted per feature.
- [ ] Complete direct engineering and end-only Fable whole-roadmap reviews, dispose every finding, approve/archive unchanged under the owner's grant, then write EP022 in full and continue.

## Surprises & Discoveries


The protocol already has the phone's compact tool fields in crates/carlos-proto/src/session.rs. The missing part is the owner producer: crates/carlos-core/src/session/event_rows.rs:107 currently derives only a coarse kind/failure state and sets summary/added/removed toNone. Fill the existing fields before proposing another wire record.

The red actual-daemon fixture publishes a command with explicit exit0, a Unicode diff with one added/removed line and a completed command with no observed exit. The existing producer reports the command asOther. An initial attempt to place facts in ToolOutputDetails changed legacy labels when generic tool calls gained metadata; two existing native-history/id-less golden tests caught that in tool-facts-rust-03.log. Store optional boxed ToolFacts directly on Message/EntryDraft instead. This keeps native facts independent of literal/generated labels and requires no parallel transcript index. The unchanged legacy assertions pass in tool-facts-rust-04.log.

The new command fixture originally expected bare aggregatedOutput as its source, but existing command source intentionally includes the literal command and exit details. Preserve that exact source; derive the two-line compact summary from native output instead. The corrected fixture still fails on the accepted baseline for missing Command kind, /tmp/carlos-ep021b/tool-facts-before-correct-source.log, then passes against the new producer. No source formatter or original-byte assertion was weakened.

Codex has a complete reader in protocol_history.rs, with explicit unsupported-method fallback, repeated-cursor checks, duplicate native-id handling and aggregate limits. It is also used by rewind/fork validation, so replacing it globally with a tail-only reader would break ancestry and must not happen. Add an initial-window/read-older path while preserving complete reads where a deliberate fork/reload needs them.

Claude's helper already keeps a read-only SDK-imported archive, a snapshotId and bounded sessions.read pages, including chunked long records. Its SDK import/projection can still scan the complete native session before a page is available. A paged wire response is not proof that this native startup cost disappeared. Grok loads history through native ACP replay; do not invent an unsupported native paging method or substitute a guessed local history format.

## Decision Log


Decision (2026-11-16, Root): implement the clearly missing compact tool facts first, then actual Grok approvals, then the native history window work. These are independently buildable milestones under one phone-data goal. Do not request intermediate reviewers or add Swift code before the whole plan is ready.

Decision (2026-11-16, Root): cache native kind/outcome and result counts when a tool item changes, in optional boxed ToolFacts on the existing Message/EntryDraft. Keep ToolOutputDetails unchanged as the legacy label record. A completed native item without exit/success evidence remainsUnknown. A later AI label changes only the label; it must retain native kind/outcome. Existing Tool/EntryFacts fields carry these values without a new wire family.

Decision (2026-11-16, Root): use supported native page/cursor APIs and the existing Claude SDK archive. Codex's public pages must support a bounded newest window plus explicit older reads. Claude may page its checked archive without claiming to eliminate the SDK's initial import cost. If Grok's installed/published ACP interface requires complete replay, retain that honest native startup behavior and provide the existing complete client history pages after replay; do not fake a cursor or drop older data. Record each backend's measured behavior and remaining constraint. Improve supported paging without breaking compatibility or inventing backend features.

Decision (2026-11-16, Root): continue EP021a's first-phone binding boundary. Existing shared queue/context-default/goal/task reader families stay Rust-only until actual screens need them. Track added/deleted production code per feature and justify net growth; do not enforce an arbitrary line quota by weakening correctness, ancestry or complete data.

## Outcomes & Retrospective


The actual-daemon regression fails against accepted EP021a for the intended missing native tool kind and now passes against the new producer, including source bytes. The full Rust suite and existing legacy history/id-less checks pass after separating native facts from labels. Daemon188 and selected terminal20 checks, strict lint and shape checks pass for the first cache/export piece. It adds169 production lines, allCore, with no new wire/client layer. Remaining tool work includes backend metadata preservation and negative/history cases before the full script gate; approval/history milestones have not started. No new reviewer or developer worker has run. Finish each milestone's evidence and delete replaced paths before the final whole-plan review.

## Context and Orientation


carlosd is the host process that owns native sessions and serves typed JSON-RPC over Unix sockets or authenticated WebSockets. One carlos-core owner thread mutates each SessionState. Captured BackendCommands handles let bounded workers perform native I/O while that owner handles events and urgent controls. Private permissions and connection/root generations stay in the daemon/core, never in a caller-provided wire payload.

carlos-proto holds wire data. Its session::Tool contains ToolKind, ToolStatus, label/label_source, optional summary/added/removed, has_output and output_revision. An Entry identifies the tool and points to separately fetched original source/output. core session/event_rows.rs exports compact facts; session/models.rs holds the owner's ToolOutputDetails. Native item/output handlers and pure tool_command_view/tool_source/tool_diff/tool_diff_summary/tools helpers already compute terminal-independent facts. Reuse them where their meaning is correct, without interpreting colored terminal output or replacing literal source with generated labels.

carlos-client owns device-local checked copies, request ids, body windows and numbered history settlement. It owns no sockets, clocks, native backends or terminal widgets. client_history.rs holds one bounded waiting-page mechanism; history.rs validates prepended pages against the original before EntryId. Effects is must_use, the panel reply router is exhaustive and carlosd/command_info.rs exhaustively assigns every command its handler and allowances. Preserve these compiler guards and explicit cancel/disconnect/byte-charge hooks if a new read result is needed. Prefer extending the existing history path over adding a HistoryPanel framework.

The terminal Device keeps private input, scroll, render caches, saved edit intent and consent. src/app/remote/reading.rs requests older entries through the client; no native cursor or backend JSON belongs there. The terminal's current full source/color/copy behavior must remain unchanged while headless clients receive richer compact tool data.

For Codex, core/codex_snapshot.rs reads initial/reconnect state and calls protocol_history::load_thread_history. That reader uses thread/turns/list, thread/items/list and an explicit old-server thread/read fallback, draining all pages before replacement. The same complete reader appears in protocol_rewind.rs and protocol_rewind_guard.rs. session/item_history.rs and history_identity.rs preserve native turn/item identity while building/adopting transcript drafts. engine/owner.rs::history and session/event_reads.rs currently serve earlier rows from the already loaded transcript; carlosd/manager/read_commands.rs routes Command::History through EngineCommand::History. This is the point where an exhausted in-memory prefix must ask the captured native source for its next older window without blocking the owner loop.

For Claude, helpers/claude-controller/src/sessions.ts serves sessions.read against SessionArchive in session-archive.ts. It imports with the published SDK, keeps read-only branch/child metadata in a temporary SQLite projection, assigns snapshotId and chunks long records. Do not write native Claude JSONL files, replace the SDK ancestry projection with a guessed parser or resurrect sidechain/child-only rows as main transcript. The Rust event normalizer/emitter orders replay and native events; preserve its captured epoch and batch rules.

For Grok, core/grok_backend/mod.rs passes a hardcoded _meta.yoloMode:true during session creation/load. process_reader.rs handles incoming ACP requests on stdout, and process_client.rs currently calls permission_allow_reply immediately, choosing an allow option or inventing allow-once. Requests are also currently recognized through numeric ids before method routing. BackendCommands already exposes respond/respond_error; extend the captured Grok endpoint rather than add a second native owner or read loop. Core's shared approval state, native identity checks and engine control workers provide the existing first-answer-wins path for Codex/Claude.

## Plan of Work


### Milestone 1: pin the exact native contracts and source ledger


Inspect the existing ToolOutputDetails and native describe-output/start/completion handlers, then name which native facts support each existing Tool field. Inspect the installed/generated Codex schema for descending turns/items and cursors, the pinned Claude SDK/helper archive contract, and Grok's installed/published ACP permission options. Use local code/schema/contracts first; if outside documentation is needed, use primary official sources and record the exact version/link. Do not guess a native method or policy value from memory.

Create a short feature ledger in .agents/reports/exec021b-source-ledger.md. Baseline authored production source is94870 nonblank lines, excluding tests/examples/build scripts/docs/generated data; client10263, core46623, proto2420, daemon8318, terminal25593 and helper1653. For every feature record which production files grew, which old path was removed/reused and why the first phone needs the change. The ledger is evidence, not a correctness-limiting quota. Reuse the accepted source-count method and preserve its scope.

Add an actual-daemon regression for at least a native command and diff whose shared facts are currently incomplete. Assert ToolKind, observed status, exact short summary/counts and original source fetched independently. Keep the fixture deterministic and show that the old producer fails for the intended missing field, not a timeout or compilation error. Do not add tests that merely repeat a helper's implementation.

### Milestone 2: export accurate compact tool facts


Fill the existing session::Tool fields from checked native data stored with the owning entry. Keep original tool identity/kind and outcome separate from an AI label: a later label must not turn a search into Other, change native output or invent success. Preserve Running, explicitly observed Ok/Failed/Interrupted and honest Unknown. In particular, a generic done bit by itself is not proof of success or interruption.

Reuse pure Core classifiers, summaries and diff helpers. Store/cache the small facts when the native item/output changes so ordinary export, client moves and paints do not rescan large bodies. Native provider facts win; a derived line count or short literal result must be based on actual output. Unknown counts remainNone; a real zero remainsSome(0). Do not generate new model inference for result summaries or parse terminal glyphs/colored labels in a phone/client.

Publish compact changes through the existing EntryFacts/numbered event path for every subscriber, while body pieces remain on bounded source/output reads and optional live watches. Keep exact source bytes, reasoning/commentary roles, literal/AI label distinction, failed diff-command handling and running-tool expansion. Change terminal adapters only where they must consume shared facts; do not keep a second newly synchronized metadata authority there.

Run cargo test after meaningful changes and the affected actual daemon/terminal workflows. Add negative and late-update cases for unknown outcomes, AI-label updates, native failure/interruption, diff zero/nonzero counts, long Unicode labels and two clients watching different body windows. Commit this buildable feature and its ledger entry before the approval work.

### Milestone 3: route real Grok permission choices through the owner


Stop forcing bypass mode automatically during Grok new/load; honor supported native policy instead of a hardcoded true override. Recognize agent-originated requests by method plus original JSON-RPC id before the numeric response-id path. Preserve exact integer/string ids and the originating native session/connection. Other unsupported agent methods still receive an honest method-not-found response with their original id.

Translate session/request_permission into bounded shared approval facts with the native command/title/reason and only the options actually offered. Capture original option ids and native root/connection scope privately. Remove permission_allow_reply and its invented fallback; neither a malformed option set nor a missing supported choice may silently become Allow. Offer once/session/deny only when the native option set supports that meaning. An unsupported form must remain visible and safely decline/cancel-able rather than manufacture content or permissions.

Reuse the owner approval queue and the existing urgent Command::Approve lane. The worker must send the chosen original option to the captured Grok process once, with DispatchGuard/target checks through actual writer dispatch. Do not hold translation/owner locks during a wait or reuse a newly selected native session for an older card. The first valid decision wins; other clients see the shared resolved card and cannot answer the replacement. Native cancellation, turn completion, EOF, explicit stop, reconnect and fork/root replacement retire the right card and preserve private drafts. Failed/uncertain writes remain uncertain and never retry automatically.

Prove the path through the actual Grok process fixture, daemon sockets and terminal screens. Cover integer versus string ids, all offered choices, missing/unknown choices, two-client races, blocked/failed writes, interrupt while approval waits, startup callbacks, native cancellation and old-root/helper replies. Assert exact native option ids and absence of session/prompt/ordinary chat for an approval. Keep Codex and compiled-Claude approval/startup contracts green. No new permission UI or schema framework is required.

### Milestone 4: load a complete newest native window, then read older windows on demand


First make a narrow prototype/test of newest-first Codex native paging against the existing public turn/item fixture and installed contract. Retain a captured root/connection and native older cursor in Core, not the UI. A held second older page must not delay the complete first window or urgent controls. Establish the exact overlap/order/large-item rules before changing startup adoption. Keep the existing full reader for explicit ancestry/fork/legacy reads; do not globally change load_thread_history into a partial result.

Wire the checked initial window into CodexSnapshot and SessionState. Ready still means the initial native window and its source/metadata are fully adopted, never an empty placeholder that later overwrites accepted human work. Snapshot.has_older must include the real native cursor even when only the newest window is loaded. Preserve stable EntryId/native turn+item identities across overlaps, older insertion, live events, reload and rewind. An older page is historical data, not a new live message or approval. It must not start tool-label inference, duplicate local echoes or replay old actions.

Extend the existing owner/manager History read path at the loaded prefix. Native I/O runs on an existing bounded captured worker while the owner handles events/controls. Retain the exact requesting session/before id, reader interest and root/connection target until completion; a disconnect, replaced root, stale cursor or failed read must release its original permit and leave the accepted visible page/private composer intact. Multiple clients retain separate before cursors and scroll positions. Merge older entries without replacing or reverting newer streamed facts. Preserve the client's one numbered waiting-history path and before-id validation; do not add a generic controller or make the terminal assemble native pages.

Use the existing Claude read-only SDK archive/snapshotId and bounded long-record pages to request a newest projected window followed by earlier offsets where its API supports that order. Preserve compaction ancestry, exact native UUIDs, main-versus-child visibility and fork cuts. Measure the SDK import/projection separately and explicitly retain any unavoidable initial scan. If an archive expires or the native file/root changes, report stale scope and keep the page; do not silently join different snapshots. Use supported native Grok behavior: if ACP load must replay the entire session, preserve it and expose complete client pages after replay. Do not claim that backend's startup became lazy or invent a paging method/local parser.

Keep supported older data accessible; do not truncate bodies, hide has_older or lower counts to improve a benchmark. Preserve existing old-server fallback only for explicit unsupported methods, not permission errors, malformed pages or repeated cursors. A native cursor/revision/root error is a visible read failure. Reuse stored bodies and stable ids rather than clone the whole transcript per page. Record remaining native-specific limits honestly. Update architecture/history documentation to state that a complete initial native window is required before Ready and older native windows are read only on demand where supported.

Add actual daemon and terminal workflows for a multi-page native session with a deliberately held older read, concurrent streaming/control/approval, two independent clients, UTF-8 long records, repeated/foreign cursors, duplicate ids across different turns, reconnect/fork while a read waits and retry after explicit failure. Assert visible anchored content and exact native requests, not just decode success. Existing rewind, source/color/copy, startup callback, Claude compaction/sidechain and Grok replay cases must remain green. Add a test-side Rust consumer of the same history path if the native contract changes need proof beyond the terminal.

### Milestone 5: measure, review and hand off to the actual phone


After every feature is connected, delete replaced paths and complete both Rust configurations, both strict lints, architecture/dependency/shape checks and the complete stock release gate with Codex/Claude contracts. In this root shell copy scripts/test.sh privately and change only its cwd/artifact root to /tmp/carlos-ep021b, as in the accepted gates. Keep original assertions/deadlines. Preserve and investigate the first failed trace before rerunning; no review worker runs until the whole plan is otherwise ready.

Only after all builds/tests/hooks finish, freeze matching CLI/daemon/helper/probe files and their source/hash identities. Compare against /tmp/carlos-ep021a/reviewed-pair/identity.json. Run15 alternating renderer samples per binary/case on CPU6 with the same1000 turns/seed1/tool-lines24/160x48 and the captured Core fixture; repeat three actual1000-prompt Unix/WebSocket admissions and exact10092544-byte/155-chunk source transfers per binary/transport. Preserve source hashes/counts and report p95/max, fixed RSS, binary sizes and the per-feature production ledger. Use carlos perf-session, not subjective typing impressions. The original EP016 rule blocks median slowdowns above10% until explained; its baseline cold replay is49.27ms, scroll p950.84ms and typing p950.63ms. Also compare closely with EP021a's fresh matched figures.

Add a real native-history startup/read measurement: large deterministic native history, a small newest window and a held later page. Record time to first complete visible window, native pages fetched before Ready, older-read latency, peak/retained RSS and typing/interrupt while the older read is held. Compare complete traversed entries/source/native identities, not a faster truncated first page. Report Claude archive import and Grok replay costs separately where native behavior prevents lazy startup. Keep the inherited dense-Unicode terminal cost and unexplained earlier500ms write timeout visible; this plan cannot hide either through a body cap or longer deadline.

At the end perform the direct engineering review and one Fable5.2 medium whole-roadmap review of the complete feature set and first-phone binding boundary. Record every decision, address concrete defects and avoid speculative refactor/reviewer loops. Once all checks/reviews pass, record delegated approval, finish the four living sections, archive this file unchanged, update PROGRAM_PLAN.md, write EP022 in full and continue. No install/merge/push or routine human gate is implied. EP022 must build/test the actual Swift consumer before adding bindings for unused Rust families.

## Concrete Steps


All shell commands run through rtk from /var/home/wegel/work/perso/carlos-daemon. Read AGENTS.md, .agents/CODE_STYLE.md and the relevant native producer/validator before editing. Use semeja search/find-related for intent and exhaustive literal searches only where needed. Keep scoped imperative commits small and buildable.

    rtk cargo test --workspace --all-targets
    rtk cargo test --workspace --all-targets --no-default-features
    rtk cargo clippy --workspace --all-targets -- -D warnings
    rtk cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy python3 -m unittest discover -s tests/architecture -p 'test_*.py'
    rtk proxy python3 scripts/check-core-dependencies.py
    rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract

Use the private cwd/artifact-only adaptation for the final script in this shell. Keep logs/native frames/screens/keystrokes under /tmp/carlos-ep021b and record exact targeted commands in Progress as tests are added. Before meaningful runtime edits create the tool-fact red regression, then implement and run the actual affected workflow. Before native-history changes, prove descending native pages/ancestry in the small prototype above. Review only after all feature gates and measurements.

## Validation and Acceptance


A headless client sees correct command/read/search/edit kinds, native outcomes, short output summaries and real diff counts without parsing original tool text. A label update preserves native type/outcome/source. Two clients receive the same compact facts and retain different expanded body windows. Running tools still expand, source/color/copy behavior remains exact and unknown outcomes remain unknown.

A Grok permission request waits for an explicit offered choice. Any client can send that choice through the shared approval command, exactly one native response wins, all clients see the resolution and no ordinary prompt is sent. Old callbacks cannot act on a replacement root/process. Native error, interrupt, cancellation and startup paths remain responsive and keep private drafts.

A large supported Codex native session opens from its newest complete window without reading every older page; reaching the top requests the next older page and preserves the visible anchor. Complete traversal returns every supported native identity/source once. Controls and streaming remain responsive during a held older read. Claude/Grok keep exact native history and honest capability-specific startup behavior, without guessed pagination or hidden truncation. The existing shared client performs numbered merge/before-id checks once; terminal and future phone code do not copy native assembly.

All final checks and source-matched measurements pass with costs explained. Main, installed Carlos, archived EP021a/EP020 and both frozen owner previews remain unchanged. Reviews find no unresolved blocker/code defect, and the next artifact is a full EP022 plan for the actual app, not another unused binding layer.

## Idempotence and Recovery


Use fresh private fixture homes and exact test-owned PIDs. Never stop the owner's long-running Carlos/Codex processes. Do not write native Claude/Grok history files, overwrite accepted pairs or edit archived plans. If /tmp baseline files are lost, rebuild the checked3ffd3eb tree in a separate detached temporary checkout using the pinned toolchain/locks, run its gate and freeze its matching package before comparison; do not check out/reset the active branch or touch protected main.

A failed native read preserves the accepted page and original private input, releases its permit and requires an explicit read retry. A failed possibly-written mutation stays uncertain and is never resent automatically. A stopped session/native root retires scoped readers/callbacks. Record discovered native limitations and changed assumptions in this plan rather than silently falling back to unrelated data. Small buildable commits allow a feature correction without reopening the accepted common-reader extraction.

## Artifacts and Notes


Accepted baseline: source896435d22d03d85dba4cef0f58ba68e8, checked tree3ffd3eb, closure4407f6e. Runtime review correction06f13ce. Identity /tmp/carlos-ep021a/reviewed-pair/identity.json; final report/measurements/review decisions .agents/reports/exec021a-*. Full baseline gate release-gate-04.log, artifacts release-gate-FWCma9. Archived EP021a SHA2562104819e6ca9f8eb4f6648a2efdf2645d50a1778db4abadb240e2e3d51d632a0.

Baseline matched synthetic cold replay50.56ms, scroll/typing median per-run p950.69/0.48ms; actual prompt admission medians about0.552ms Unix and0.703ms WebSocket. Exact source is10092544 bytes/155 chunks, SHA256 f61a23673ac06e7f0d863d63cf117229db92782f5a5cf48d4472f3787741d73e. CLI17499736 bytes, daemon20158568, helper104780090. The source ledger starts at94870 nonblank authored production lines.

Protected main remainsa29b1a1e3720e48b09e2ca984fb16c29b5857ea3 and installed Carlos SHA256d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3. Frozen previews are target/preview-e6be8122 and target/preview-9cc99ff. The latter is the fully checked CLI offered to the owner, predating only the narrow final compiler/API corrections.

## Interfaces and Dependencies


Prefer the existing Tool/EntryFacts/History wire types and Client history/body/approval APIs. Keep version1 provisional and document any necessary tested extension. New commands must receive an exhaustive daemon description; new read replies must receive an explicit route. Use named payload records for new writes while keeping captured owner/reader/connection guards private. Do not add a generic command builder, generic panel registry, socket/clock/thread dependency to carlos-client, new async runtime, native owner or new model inference for summaries.

A native history window must carry its captured backend/root/connection and actual older cursor privately in Core. A public before EntryId remains a checked Carlos identity, never a raw native token. A Grok approval retains the exact native callback id and offered option ids privately until its original guarded response. Compact tool facts derive from the one stored native source and checked outcomes. These constraints protect the first Swift/web consumers without requiring either consumer or bindings in this plan.

Revision2026-11-16: create the next full plan after delegated EP021a closure, incorporating the final first-phone binding boundary, narrow source-growth ledger and measured baseline. Establish actual producer gaps before code; no new reviewer/developer worker is used.
