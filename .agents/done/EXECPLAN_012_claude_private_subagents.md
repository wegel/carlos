# Keep Claude child workstreams out of the main conversation

## Goal

Carlos should show the main Claude assistant's conversation. Child text, thinking,
tool calls, tool results, and raw child report notices belong outside that view.
F2 must retain task details and stop controls. Native questions must remain answerable.
This task runs in Discussion Mode, with one implementation agent and parent review.
The parent approved the implementation and explicitly authorized the paired local
install. Do not commit.

## Progress

- [x] Read the live, history, task, and callback paths at clean baseline `1ff03fd`.
- [x] Agree scope with the parent before the full test run.
- [x] Share native ownership checks across live events, history, and transport guards.
- [x] Remove the child transcript renderer and its lifetime cursor cache.
- [x] Keep task details in F2 instead of adding verbose main transcript rows.
- [x] Finish focused Rust and terminal checks.
- [x] Run the required release suite and compare transcript performance.
- [x] Hand off the stable package and evidence; parent review PASS.
- [x] Install the approved regular pair and verify actual installed workflows.

## Surprises & Discoveries

- Carlos previously rendered child events as tool output and stored a translation
  cursor per child. That cache could close a healthy session after 128 concurrent
  child identities. Hidden child output needs neither the renderer nor that cache.
- Loaded history already skipped two parent ID fields but missed `agent_id` and
  the older explicit `isSidechain` flag.
- The SDK history view can supply null parent IDs even when imported raw records
  contain real IDs. The helper must preserve those raw facts during its merge.
- Native task events and permission callbacks use separate paths. Carlos can keep
  them usable without adding any child tool or assistant rows to the main view.
- A native `task-notification` origin can also describe scheduled or external
  messages. Carlos must not hide every message with that origin kind.

## Decision Log

- Hide child envelopes only from positive native fields: `agent_id`,
  `parent_tool_use_id`, `parent_agent_id`, or `isSidechain: true`. Do not inspect
  text or tool arguments to guess who owns a message.
- Hide raw user report notices only for `task-notification` with a nonempty `runId`
  and no subkind, or peer/observer origins with a nonempty `senderTaskId`.
  Keep main assistant reports, human text, Ralph prompts, scheduled messages,
  cross-session peers without a sender task ID, and unknown notices.
- Keep raw helper frames and loaded records intact. Only the UI's main transcript
  excludes these records. The helper preserves raw ownership fields over null values
  from the SDK history view; no native CLI, authentication, or SDK option changes.
- Process task state and browser completion before excluding child conversation
  events. Child init/reset frames must not alter the main session identity.
- Keep Codex and Grok behavior unchanged.

## Validation

- `cargo test claude_ --quiet`: 92 passed, 397 filtered.
- `bash scripts/build-claude-controller.sh`: 52 source and 52 standalone helper
  tests passed, 284 assertions per run. The new test exercises the official SDK's
  history reader and preserves raw agent/parent IDs over projected null parents.
- Three focused terminal cases passed at `/tmp/carlos-private-children-focused-fixed`:
  main tools plus hidden late child snapshots/streams/tools; child question and
  native stop; 180 hidden children with a usable composer; resumed records with
  private child work and an unchanged main report/tool output/rewind.
- The first terminal run at `/tmp/carlos-private-children-focused` exposed dropped
  parent IDs in helper history. The helper fix preserves those fields; the same
  assertions then passed. The earlier focused Rust run caught missing role fields
  in the new main-tool test fixture; correcting that fixture restored valid wire.
- `cargo fmt --check`, `check-rust-style.sh --all`, and `git diff --check` passed.
- `bash scripts/test.sh --release --codex-contract --claude-contract`: PASS.
  Artifacts: `target/ui-artifacts/run-bFzukW`. The run passed strict Clippy,
  488 Rust tests (one ignored), 52 source plus 52 standalone helper tests
  (284 assertions each), package smoke, 118 terminal workflows, Codex contracts,
  three native Codex monitors, and four native Claude localhost cases.
- Five alternating, sequential release comparisons against a preserved copy of
  the installed Carlos passed. Same synthetic inputs: 1000 turns, 24 tool lines,
  seed 1, 160x48. Median before/after: replay 48.50/48.70 ms; full layout
  15.02/15.19 ms; scroll p95 0.82/0.82 ms; typing p95 0.63/0.60 ms. Both render
  39347 lines. No material regression. Script, individual runs, baseline hash,
  and results: `/tmp/carlos-private-children-perf/`.

## Reviewable package

The Linux pair is staged at `target/ui-artifacts/run-bFzukW/package/`, with
410 files including notices, rebuild sources, build manifest, and checksums.

- Carlos SHA256: `230bffc131b979e16f84957e10b6332be9f7b384b2fa05a3faec038ccf21131c`.
- Helper SHA256: `a44f4b3e02aeeac1f314eabd1c8ad1aac06b291a204fbd342d35944ac7c7311a`.

The helper changed to preserve raw history ownership, so the parent authorized a
paired install after reviewing the release package and evidence. No commit was made.

## Parent review and installed checks

The parent reviewed the production handlers, shared live/history owner checks,
helper merge, regressions, release package/native evidence, and performance.
Verdict: PASS, no findings. The parent independently verified package hashes,
whitespace, and the installed file hashes/types/modes.

- Durable package, notices, rebuild sources, and manifest:
  `~/.local/lib/carlos/releases/0.1.0-private-subagents-230bffc1/`.
- Previous regular pair and prior install metadata:
  `~/.local/lib/carlos/backups/pair-before-private-subagents-20261111T040600Z/`.
  Prior Carlos hash starts `d00a4c1c`; prior helper hash starts `278fdcc4`.
- Installed regular executable files, both mode 0755: `~/.local/bin/carlos` and
  `~/.local/bin/carlos-claude-controller`. Their full hashes match the package
  hashes above. The installer staged regular files, replaced helper first, then
  Carlos, and verified 409 package checksums (410 files including SHA256SUMS).
- `--help` and all three focused installed workflows passed: child output remains
  hidden while main tools, child callback and stop controls work; 180 hidden
  children leave Carlos healthy; resumed child records stay out of main history
  and rewind while main reports/tools remain.
- `/proc` checks proved each test used the actual installed Carlos and helper
  paths. No helper override or copied test binary was used. Homes/configs were
  isolated; all native peers were deterministic fixtures, with fake credentials
  and a localhost-only API address. No paid or account inference ran.
- Driver: `/tmp/carlos-private-subagents-installed-smoke.py`.
  Screens, helper/native wire, process-path proof, help output, and result:
  `/tmp/carlos-private-subagents-installed-gspe4y9_/`.
- Install metadata: `~/.local/lib/carlos/current-install.json` and
  `target/ui-artifacts/run-bFzukW/install.json`.
  Earlier backups remain intact; native Claude and user settings were not changed.

All model fixtures run locally without account inference.

## Outcomes & Retrospective

The implementation, required checks, parent review, paired install, and installed
smoke are complete.
Carlos no longer allocates child transcript cursors or creates task-detail rows in
the main conversation. Native task state and callback requests still reach F2 and
the question UI. Main tools, main assistant reports, and known human history stay
visible and editable.

Carlos uses native ownership fields rather than text guesses. Old records with no
child ownership or positive report origin remain visible because Carlos cannot
safely distinguish them from ordinary main conversation records. F2 retains task
metadata; this change does not add a full child transcript browser. Codex behavior
and native CLI/authentication/settings are unchanged.
