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
- [ ] Run full tests, strict compiler checks, terminal checks, direct engineering review and measured performance comparisons; resolve failures.
- [ ] Document the frozen contract, record evidence, move this completed plan unchanged to `.agents/done/`, and stop for owner review before EP019.

## Surprises & Discoveries


The core already owns backend handlers, but `SessionUpdate` and `TranscriptChanges` report only redraw hints. They do not describe client facts. Approval cards use stable entry ids but live separately from transcript rows. History replacement swaps the Transcript object, so a session clock must live in SessionState. The core uses native thread ids internally; clients need a separate stable session id that survives a fork. The phone needs forms of approval beyond shell commands because the existing terminal supports questions and permission requests.

## Decision Log


Decision: Work only in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`; do not install, merge or push. Stop after EP018 for owner review. Rationale: the owner chose this checkout and branch and the architecture requires review before installation and the next EP. Date/Author: 2026-11-13, Codex.

Decision: Work and review directly without child agents or external reviewer sessions. Use `.agents/reviewers/engineering_reviewer.md` as a self-review checklist and label the result honestly. Rationale: the owner explicitly requested no subagent. Date/Author: 2026-11-13, Codex.

Decision: Add a pure `carlos-proto` crate with serde types. Keep socket listeners, the replay ring, shared client state machine and multi-client command arbitration for EP019. Add JSON Schema generation in EP020 as scheduled. Rationale: EP018 must prove meaningful client events without moving future server work into this step. Date/Author: 2026-11-13, Codex.

Decision: Keep opaque binary EntryIds inside the core and create wire strings only when a caller exports data. Publish borrowed event batches for the terminal. Record changes at transcript writers rather than comparing cloned transcripts. Coalesce writes before a publication and bound pending records; a reset replaces an overflowing batch. Rationale: text streaming must send only new text and must not copy growing history on each fragment. Date/Author: 2026-11-13, Codex.

## Outcomes & Retrospective


The protocol crate passes its contract tests, including frozen command and notification examples. The core journal passes six headless tests and the full Rust suite. The terminal migration is in progress; the old return types were removed in commit `d9db5ab`. EP019 will add a daemon process, sockets and reconnect replay. EP021 will add backend history paging and richer tool summaries; this contract must represent unknown facts honestly until then.

## Context and Orientation


The Cargo workspace contains the terminal binary and `crates/carlos-core`. `src/app/state.rs` owns a core SessionState and a terminal ViewState. The core stores rows in `crates/carlos-core/src/session/transcript.rs`; each Message has an opaque EntryId and text. The core also stores approval cards and session details. Backend handlers currently return SessionUpdate values identifying the earliest dirty row, and the terminal clears render caches from there. These old reports cannot serve a phone.

`docs/engine-architecture.md` schedules EP018 as the third of seven steps. The phone requirements in `tmp/main-screen.md` section 11 require entries, streamed text, tool state and labels, approvals, phase, model, context, quota and turn start time. A snapshot is an immutable view of these facts at one event number. A numbered event describes a change after that snapshot. A reset tells a client to replace its copy from a new snapshot. A publication groups pending changes while borrowing the session so a caller cannot change text before encoding the batch.

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


Pending direct review against `.agents/reviewers/engineering_reviewer.md` after the source and evidence are ready. No independent reviewer will be claimed.

Update note (2026-11-13): created EP018 after the owner's approval of EP017; recorded scope, proposed interfaces, measurements and review boundaries before source changes.

Update note (2026-11-13): recorded the first protocol and core journal checks; the terminal now has an event consumer under test before removal of legacy reports.

Update note (2026-11-13): removed legacy redraw returns and migrated their exhaustive tests to numbered records. The full Rust suite and 26 architecture tests pass; strict lint and full terminal checks are next.

Update note (2026-11-13): the first release gate passed all 174 terminal workflows, 23 native-monitor checks, installed Codex contracts and helper/package checks. The wire review then added neutral question fields, explicit unsupported input, optional accepted approval decisions and stable next-card ids for focus moves. New JSON fixtures cover every version-one command and event. These refinements require final tests and a fresh measured executable. Both core and protocol package guards now reject transport/UI packages in the protocol graph.
