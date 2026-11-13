# EP018: numbered core events and client protocol version 1

Final gate status: PASS, 2026-11-13 15:07 UTC. The final release runner passed, and its executable hash matches the measured binary.

## Result

The core publishes numbered changes to rows, live approval cards and session details. The terminal consumes those records directly and resolves stable ids when clearing its render cache. The old SessionUpdate and TranscriptChanges types and their return plumbing were deleted in commit `d9db5ab`. A client can also serialize a sealed batch, read a snapshot, page through stored rows and fetch tool output without terminal types.

`carlos-proto` defines version-one JSON-RPC envelopes, commands, replies, opaque addresses, lossless decimal-string cursors, snapshots and events. `docs/client-protocol-v1.md` specifies their meaning. The golden fixture covers every command and event. The core uses its existing readers to describe Codex and Claude questions through the same neutral fields; it also reports unsupported inputs, browser consent, negative defaults, offered choices and accepted decisions without exporting backend routing payloads.

A caller holds a borrow of SessionState while it reads or encodes a publication. The core can coalesce several suffix writes without cloning the existing text, and it cannot mutate that text before the caller finishes encoding. A transcript journal holds at most 256 distinct pending row edits. Large bursts and history changes produce one reset. The session clock lives outside the transcript, so native forks and history swaps do not restart it. The terminal keeps its full borrowed transcript and its current cache; it does not create another transcript copy or serialize every local event.

## Source and binaries

Work stayed in `/var/home/wegel/work/perso/carlos-daemon` on `iphone-spike`. The reviewed runtime source is `fc7afb846a5829bfc74a5c67267d9452a4430d3c`. Later closeout commits change tests or documents only. The branch contains `main`; no main changes required a rebase during this step. Nobody merged, pushed or installed this spike.

The compiler was Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5). The accepted default release executable is 20,146,232 bytes, SHA256 `ae5b517c49bba678c6d6b9f6513ce2e3bd4e3200daaec22cff203747e3c94ca8`. EP017's executable was 20,135,896 bytes, SHA256 `a8cd1d59293c0108c7942bde66156ed21155a3a1fa67cb91fc014a70bc803078`. The new executable adds 10,336 bytes, about 0.05%. The no-default-features release also built; its hash is `2845bbb1442958a113d9ecbb83e4225deb5d914d9ddbd692fba6c401587ae769`.

The original pre-plan executable remains the architecture reference: SHA256 `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`. Raw provenance and timing files live under `/tmp/carlos-ep018/accepted-source.json`, `accepted-holdout/`, `accepted-rss/`, `rss-repeat/` and `rss-breakdown/`.

## Checks

The final source passed `cargo test`: 811 application tests, 231 core tests and five protocol tests, with three explicitly ignored larger audits. Two private-field compile-fail doctests also passed. Without default features, 775 application tests, 231 core tests and five protocol tests passed, with two ignored audits and the same two doctests. Strict Clippy passed for both feature sets. Both release feature sets built, and the default executable was restored before terminal checks.

The new core tests reconstruct a headless client's copy from serialized real events, including through the actual backend handlers for Codex, Claude and Grok. They check Unicode streaming, normalized reasoning, row replacement, completion, labels, approvals, queue focus moves, browser choices, permission choices, resets, stale addresses, pages and bounded bursts. The client copy must equal a core snapshot after each batch. The protocol fixture also rejects an unknown event kind, so a client cannot silently advance past a state change it does not understand.

All 27 architecture/package Python tests passed. The source guard and resolved dependency guard passed, including the new protocol graph check. A standalone core check produced 161 compiler artifacts with empty core features, no test-support feature and no terminal, audio or speech package. Changed Rust files are at most 358 lines and functions at most 60 lines.

The final `bash scripts/test.sh --release --codex-contract` passed all 174 terminal workflows, installed Codex contracts, 23 native-monitor workflows, 59 helper tests, terminal-recorder checks and standalone package smoke checks. Artifacts live in `target/ui-artifacts/run-ZoB6us`; the full log is `/tmp/carlos-ep018/accepted-release-contract.log`. The installed Codex CLI was 0.167.1. No inference or user session was required by these checks.

## Measured speed

Thirty-one quiet rounds ran the original pre-plan, EP017 and accepted EP018 executables on CPU 6, rotating all six possible orders. Each executable used the same seed, 1,000 turns, 24 tool lines and a 160 by 48 terminal. The captured stale-transcript fixture ran separately. No build or UI runner competed with these measurements. All runs agreed on 6,001 rows, 39,347 rendered lines and 6,000 relevant items for the synthetic session, and 5/9/4 for the fixture.

Synthetic medians in milliseconds, listed as pre-plan / EP017 / EP018:

- Apply: 14.17 / 14.18 / 14.19.
- Complete replay: 47.55 / 47.12 / 46.94.
- Full layout: 15.16 / 14.91 / 14.80.
- Full draw: 3.54 / 3.49 / 3.43.
- Scroll p95: 0.82 / 0.82 / 0.82.
- Typing p95: 0.62 / 0.62 / 0.62.
- Working draw p95: 0.62 / 0.61 / 0.62.
- Append p95: 0.66 / 0.66 / 0.67.

The paired apply difference against pre-plan was +0.02 ms with a bootstrap 95% interval of [-0.09, +0.13]; against EP017 it was -0.03 ms, [-0.08, +0.07]. These samples do not establish a slowdown or an exact zero cost. No measured metric had a positive lower confidence bound against either baseline. Fixture replay medians were 1.05 / 1.04 / 0.97 ms.

## Memory

Linux wait4 supplied peak resident memory; the older built-in RSS fields still report zero and were not used. The accepted five-pair sweep gave EP017/EP018 medians of 23,528/24,436 KiB at 1,000 turns, 30,292/31,184 at 2,000 and 43,700/44,744 at 4,000. Eleven further pairs confirmed a roughly fixed 1 MiB resident difference: 23,296/24,688, 30,204/31,324 and 43,712/44,836 KiB. The difference did not grow with rows.

An earlier sweep of the unchanged EP017 executable had reported higher RSS, so the review separated anonymous memory, which includes heap and stack, from file-backed pages. Five more pairs sampled `/proc/PID/status` at 4,000 turns. Anonymous-memory medians were 32,832/32,844 KiB, a 12 KiB difference. File-backed medians were 11,192/12,144 KiB. The new session bookkeeping remains bounded and adds no per-row memory slope. The fixed file-backed resident difference remains visible; this report does not claim equal total RSS.

## Direct engineering review

The owner requested direct work with no subagent. Codex applied `.agents/reviewers/engineering_reviewer.md` directly; no independent reviewer or separate reviewer session is claimed. The review covered `919a0a7..fc7afb8`, the client contract, terminal bridges, tests and measured results.

The review found and corrected these issues before closeout:

- MAJOR: reasoning normalization could turn every suffix into a full text replacement. The store now recognizes text that extends the published prefix; the Unicode regression test checks the actual suffix on the wire.
- MAJOR: a stopped turn could leave streamed rows unfinished when the backend omitted item-completed frames. The owner now finishes the active suffix through the store and emits facts while retaining partial text.
- MAJOR: moving approval focus could reorder cards without telling a client where to place them. Card events now name their next neighbor by opaque id, and the queue-move test compares the client order with the core order.
- MAJOR: using a native empty-form reply helper to list browser choices hid explicit consent. Browser cards now offer once and retain default-to-no facts. Tests cover both backends and reject an unsupported permissions cancel choice.
- MINOR: an internal approval event initially carried a list offset. It now carries the stable card id. Full replacements and optional accepted decisions also keep withdrawal distinct from user approval.

Corrective guidance: implemented for every finding above. Final verdict: PASS. No open source finding blocks EP018. The final Rust, feature, dependency, terminal, contract and speed checks pass; the memory paragraph records the fixed resident difference explicitly.

## Follow-up context

EP019 adds carlosd, the session owner, Unix sockets, a bounded owned replay ring and carlos-client. It must implement the documented neutral answer checks, input retry rules and single winning approval answer before accepting remote commands. EP020 adds WebSocket access, tokens and generated JSON Schema. EP021 adds lazy native history, Grok approvals and richer tool summaries. EP022 builds the iPhone app.

EP018 pages rows already held in memory. It reports unknown tool details honestly instead of inventing success, summaries or diff counts. It keeps live cards separate from transcript history; resolved cards are not yet persisted as historical rows. A phone history design must account for that before relying on resolved cards after a reset. The pre-existing queued-Claude-card-after-fork issue and broken built-in RSS helper remain recorded limits from EP016. The Mac was not touched.

The owner reviews each ExecPlan before the next one starts or any spike executable is installed, as required by `docs/engine-architecture.md`. The branch is ready for that review. EP019 and installation remain behind that gate.
