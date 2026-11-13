# Emit numbered session events and freeze client protocol version 1


This ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log and Outcomes & Retrospective current.

## Purpose / Big Picture


Carlos will expose the conversation without terminal widgets or raw backend messages. A client can read a snapshot, then apply numbered changes to entries, approval cards and session details. The terminal will consume the same changes directly in memory. It must preserve its current screens, backend replies and speed. This step supplies the contract for the later daemon and iPhone app; it does not start a server.

## Progress


- [x] (2026-11-13) Read the repository rules, architecture, phone screen requirements and current session writers. The owner approved EP017 and authorized EP018.
- [x] (2026-11-13) Preserve the accepted EP017 executable under `/tmp/carlos-ep018/ep017-carlos` before building.
- [x] (2026-11-13) Add and test `carlos-proto` types for ids, numbered events, snapshots and typed commands; final contract review remains.
- [x] (2026-11-13) Record row changes at writers; publish sealed numbered batches, snapshots, older stored rows and output. Six headless tests reconstruct client state through JSON. Approval and metadata changes share the clock.
- [x] (2026-11-13) Move terminal cache invalidation to numbered events; Rust parity tests pass. Full terminal workflow checks remain.
- [x] (2026-11-13) Delete SessionUpdate, TranscriptChanges and dirty-entry return plumbing in a separate buildable commit. Preserve local navigation effects and all writer side effects.
- [x] (2026-11-13 15:07Z) Pass default/no-default Rust and strict Clippy checks, both release builds, 174 terminal workflows, Codex contracts, 23 native-monitor workflows, 27 architecture tests, package checks, direct review and measured speed checks.
- [x] (2026-11-13 15:07Z) Document the frozen contract and accepted evidence; move this completed plan unchanged to `.agents/done/`. EP019 and installation wait for owner review.

## Surprises & Discoveries


The core already owns backend handlers, but `SessionUpdate` and `TranscriptChanges` report only redraw hints. They do not describe client facts. Approval cards use stable entry ids but live separately from transcript rows. History replacement swaps the Transcript object, so a session clock must live in SessionState. The core uses native thread ids internally; clients need a separate stable session id that survives a fork. The phone needs forms of approval beyond shell commands because the existing terminal supports questions and permission requests.

## Decision Log


Decision: Work only in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`; do not install, merge or push. Stop after EP018 for owner review. Rationale: the owner chose this checkout and branch and the architecture requires review before installation and the next EP. Date/Author: 2026-11-13, Codex.

Decision: Work and review directly without child agents or external reviewer sessions. Use `.agents/reviewers/engineering_reviewer.md` as a self-review checklist and label the result honestly. Rationale: the owner explicitly requested no subagent. Date/Author: 2026-11-13, Codex.

Decision: Add a pure `carlos-proto` crate with serde types. Keep socket listeners, the replay ring, shared client state machine and multi-client command arbitration for EP019. Add JSON Schema generation in EP020 as scheduled. Rationale: EP018 must prove meaningful client events without moving future server work into this step. Date/Author: 2026-11-13, Codex.

Decision: Keep opaque binary EntryIds inside the core and create wire strings only when a caller exports data. Publish borrowed event batches for the terminal. Record changes at transcript writers rather than comparing cloned transcripts. Coalesce writes before a publication and bound pending records; a reset replaces an overflowing batch. Rationale: text streaming must send only new text and must not copy growing history on each fragment. Date/Author: 2026-11-13, Codex.

## Outcomes & Retrospective


EP018 is complete on `iphone-spike`. `carlos-proto` defines client protocol version 1. The core publishes numbered, sealed batches and typed reads, and the terminal consumes their stable ids directly to invalidate its cache. Commit `d9db5ab` deleted the old SessionUpdate/TranscriptChanges path separately. The terminal keeps its current backend replies, drafts and render cache. The core emits new text only when a row still extends its published prefix, including after reasoning normalization.

The accepted runtime source is `fc7afb846a5829bfc74a5c67267d9452a4430d3c`. The default release SHA256 is `ae5b517c49bba678c6d6b9f6513ce2e3bd4e3200daaec22cff203747e3c94ca8`. The final release runner used that exact executable. Default tests passed 811 app, 231 core and five protocol cases, plus two doctests; no-default tests passed 775, 231 and five, plus two doctests. Both feature builds and strict Clippy passed. The release runner passed 174 terminal workflows, installed Codex contracts, 23 native-monitor workflows, 59 helper cases and standalone package smoke checks. All 27 architecture tests passed; a standalone core build had 161 artifacts and no terminal/audio/speech package or test-support feature. Changed Rust files and functions remain within 358/60 lines.

Thirty-one accepted CPU-pinned rounds showed synthetic apply medians of 14.17/14.18/14.19 ms for pre-plan/EP017/EP018. The paired pre-plan difference was +0.02 ms with a 95% interval [-0.09,+0.13], so these measurements do not establish a slowdown or exact zero cost. No metric had a positive lower confidence bound against either baseline. A repeated memory sweep showed a fixed roughly 1 MiB resident difference with no added row slope. A separate breakdown found anonymous memory at 32,832/32,844 KiB for EP017/EP018 at 4,000 turns; most of the resident difference was file-backed pages. The binary adds 10,336 bytes, about 0.05%. The report keeps these fixed costs explicit.

The direct review caught reasoning replacements, incomplete streams after a turn stopped, approval-order moves and browser consent choices. Each issue has a regression test. The main lesson is to record changes at the writers and publish borrowed facts, while distinguishing editor effects from conversation data. `.agents/reports/exec018-final-report.md` contains source, checks, review and raw evidence locations. `docs/client-protocol-v1.md` holds the frozen contract and the neutral answer grammar.

EP019 must add the daemon, command handlers, bounded replay and carlos-client. EP020 adds WebSocket/tokens/schema output; EP021 adds lazy native history, Grok approvals and richer tools; EP022 builds the iPhone app. EP018 pages rows already held in memory and does not persist resolved approvals as history. The older queued-Claude-card-after-fork issue and broken built-in RSS helper remain recorded limits. No Mac, installed binary, main checkout or remote branch changed. Stop for the owner's per-EP review before EP019.

## Context and Orientation


The Cargo workspace contains the terminal binary and `crates/carlos-core`. `src/app/state.rs` owns a core SessionState and a terminal ViewState. The core stores rows in `crates/carlos-core/src/session/transcript.rs`; each Message has an opaque EntryId and text. The core also stores approval cards and session details. Backend handlers currently return SessionUpdate values identifying the earliest dirty row, and the terminal clears render caches from there. These old reports cannot serve a phone.

`docs/engine-architecture.md` schedules EP018 as the third of seven steps. The phone requirements in `tmp/main-screen.md` section 11 require entries, streamed text, tool state and labels, approvals, phase, model, context, quota and turn start time. A snapshot is an immutable view of these facts at one event number. A numbered event describes a change after that snapshot. A reset tells a client to replace its copy from a new snapshot. When a caller invokes `publish`, the core groups pending changes and lends the session facts so the caller cannot change text before encoding the batch.

## Plan of Work


First create `crates/carlos-proto` and add it to the workspace. Define opaque session/entry ids, a checked sequence number, request and reply envelopes for JSON-RPC 2.0, command parameters, snapshots and semantic events. JSON-RPC means JSON objects with a method, params and request id; notifications omit the request id. Never expose vector offsets or native backend request ids. Document exact encoding, version checks and errors in `docs/client-protocol-v1.md`. Test JSON round trips and malformed input.

Next add session event modules beside the transcript. The session owns its clock and logical id. Transcript writers record added rows, changed metadata, completion and appended text; replacement and truncation request a reset. Approval writers report only actual changes. Publish final changes in deterministic order with consecutive numbers starting at one. A streaming event contains the new UTF-8 text only. Keep pending storage bounded. Export snapshots with a newest page, separate live approval cards and a sequence cursor; expose older in-memory rows and output through typed reads. Encode only when requested, so the terminal can inspect stable ids and borrowed facts without cloning rows.

Then have the terminal consume the numbered records to invalidate its render cache and keep its cursor. Preserve client-only editor effects, native reconnect frame acknowledgements, approval controls, input drafts and confirmation state. Do not serialize and deserialize every local event. Retain the old reports temporarily only to compare behavior, then remove SessionUpdate, TranscriptChanges and their return plumbing in a dedicated commit. Local focus and editor effects remain separate because they are not conversation facts.

Finally test a headless client that reconstructs a conversation from serialized snapshots and events. Check streaming Unicode, metadata-only changes, stale ids, approval retirement, history swaps and resets, and bounded batches. Run existing UI fixtures against the actual executable. Apply the engineering reviewer checklist directly and record findings. Measure the same deterministic perf sessions as EP016 and EP017 with preserved binaries, quiet CPU-pinned paired runs, and peak RSS from the operating system. Resolve regressions before closing.

## Concrete Steps


Run commands in `/var/home/wegel/work/perso/carlos-daemon`, always through `rtk`. Start with `rtk proxy cargo test -p carlos-proto` and `rtk proxy cargo test -p carlos-core` while adding the contract and events. After meaningful code changes run `rtk proxy cargo test`. Run `rtk proxy cargo test --no-default-features` and strict Clippy for both feature sets before closeout. Run `rtk proxy bash scripts/test.sh --release --codex-contract` sequentially with release builds so another build cannot replace the executable during the terminal checks.

Use `rtk proxy cargo build --release` for the measured executable. Run both `/tmp/carlos-ep018/ep017-carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48` and `target/release/carlos` with the same arguments, and with `src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48`. Save raw results under `/tmp/carlos-ep018/`. Use the earlier accepted baseline binary `/tmp/carlos-base/target/release/carlos` as the architecture reference. Record binary hashes and compiler version. Do not share Cargo target directories across different source checkouts.

## Validation and Acceptance


A headless client must receive a snapshot and consecutive events, apply them and match the core entries, live approvals and session details after streamed text, tool completion and a history reset. It must never receive native backend ids, editor drafts or terminal dimensions. UTF-8 fragments must reconstruct exactly, and completion or label changes must arrive even when text stays unchanged. Invalid ids and unsupported protocol versions must fail explicitly. A reset must contain facts at its own cursor.

The terminal must pass the existing deterministic workflow suite with unchanged backend requests and visible transcript facts. Rust tests and compiler checks must pass in both feature sets. Changed Rust files stay at most 400 lines and functions at most 60 lines. The standalone core and protocol dependency graphs must exclude terminal, audio and speech libraries. Perf evidence must meet the architecture baseline; investigate any repeatable increase instead of hiding it in aggregate numbers.

## Idempotence and Recovery


The worktree starts clean at `919a0a7`. Preserve the EP017 binary before a release build. Commands that only read/test can be repeated. Use small commits at protocol, journal, terminal, old-path deletion and closeout milestones. If a migration fails, keep the current plan accurate and fix the last buildable boundary; never reset the owner's checkout or edit completed plans. No server, user session, installed binary or remote branch is changed by this plan.

## Artifacts and Notes


The accepted EP017 executable has SHA256 `a8cd1d59293c0108c7942bde66156ed21155a3a1fa67cb91fc014a70bc803078`. EP017 passed 1,026 Rust tests, 174 terminal workflows and its dependency checks. Its synthetic apply median was 14.10 ms against 14.16 ms before the split; fixture replay median was 0.99 ms. Full prior evidence lives in `.agents/reports/exec017-final-report.md`. Preserve the `inline(always)` hints on Transcript insertion and EntryPositions insertion; EP017 measured their effect.

## Interfaces and Dependencies


`carlos-proto` depends only on serde and serde_json, with no I/O. `carlos-core` depends on that crate and retains its private EntryId allocator. New public APIs expose a logical session id, a publication containing numbered event records, conversion to owned wire events, and snapshot/history/output reads. The terminal reads records and stable ids directly, while a later daemon can serialize the same records before its next mutation. No new async runtime is needed. Check source package/dependency guard scripts and their tests when adding the first-party crate.

## Engineering Review


Codex applied `.agents/reviewers/engineering_reviewer.md` directly to `919a0a7..fc7afb8`, the contract, terminal bridges, test evidence and measurements. The owner requested no subagent, so this is a direct self-review and does not claim an independent session.

MAJOR, resolved: reasoning normalization could resend a growing body. The journal now recognizes a suffix and the Unicode test checks its wire text. MAJOR, resolved: a stopped turn could leave a row unfinished without item-completed frames. The owner finishes the active suffix and emits facts while retaining text. MAJOR, resolved: focus moves could reorder approvals without enough client data. Events now name the next card by opaque id and the client/core order test passes. MAJOR, resolved: native empty-form reply helpers hid browser consent. Browser cards now offer once and retain default-to-no facts; tests cover both backends and an unsupported permissions cancel.

MINOR, resolved: an early in-process approval record carried a list offset. It now carries the stable card id. Full replacement objects also clear nullable facts explicitly, and optional accepted decisions distinguish a user's choice from backend withdrawal.

Corrective guidance: every finding above was implemented and tested. Final verdict: PASS. No open finding blocks EP018. The final default/no-default Rust, Clippy, release, dependency, terminal and contract checks pass. Measured latency meets the agreed baseline; the report records fixed anonymous and file-backed memory costs separately. Owner review remains required before the next EP or installation.


Update note (2026-11-13): created EP018 after the owner's approval of EP017; recorded scope, proposed interfaces, measurements and review boundaries before source changes.

Update note (2026-11-13): recorded the first protocol and core journal checks; the terminal now has an event consumer under test before removal of legacy reports.

Update note (2026-11-13): removed legacy redraw returns and migrated their exhaustive tests to numbered records. The full Rust suite and 26 architecture tests pass; strict lint and full terminal checks are next.

Update note (2026-11-13): the first release gate passed all 174 terminal workflows, 23 native-monitor checks, installed Codex contracts and helper/package checks. The wire review then added neutral question fields, explicit unsupported input, optional accepted approval decisions and stable next-card ids for focus moves. New JSON fixtures cover every version-one command and event. These refinements require final tests and a fresh measured executable. Both core and protocol package guards now reject transport/UI packages in the protocol graph.

Update note (2026-11-13): direct review found that a turn can stop without individual item-completed frames. The terminal already stopped its pulse from the turn state, but a remote row could retain done=false. Finish the remaining active suffix through the transcript writer when the owning turn ends; a regression test checks partial Unicode text, completion events and idle phase. This adds no synthetic backend reply.

Update note (2026-11-13): 31 quiet CPU-6 rounds compared pre-plan, EP017 and EP018 binaries in rotating order, using the synthetic session and captured stale-transcript fixture. Counts stayed 6001/39347/6000 and 5/9/4. EP018 synthetic apply median was 14.04 ms against 14.08 ms before the split and 14.12 ms at EP017. No metric had a positive lower bound in its paired 95% interval against either baseline. Peak RSS medians at 1000/2000/4000 turns were 24600/31224/45036 KiB against EP017's 25516/32180/45960 KiB. The built-in RSS field remains a known older harness issue; these values came from Linux wait4. Raw samples, binary hashes and compiler identity live under `/tmp/carlos-ep018/`.

Update note (2026-11-13): direct review distinguished browser consent from form submission. Native response helpers intentionally cannot produce an empty form answer, so using them alone to list choices hid Allow once for browser cards. Export that consent choice explicitly, carry the backend default-to-no fact, and check both Codex and Claude browser cards. Document the neutral structured-answer grammar before the daemon implements it. Permission choice checks also avoid cloning large native payloads merely to ask whether a choice exists.

Update note (2026-11-13 15:07Z): closed EP018 after the exact measured release passed all final gates. Recorded final source/hash, direct review verdict, fixed memory costs and future boundaries before moving the plan unchanged to done. No source changes remain pending.
