# Guard Codex steer edits and label steered messages

## Goal

Carlos must not cut an entire native turn when a person selects a later message
sent during that turn. Label those messages as steers, keep their prompt text
unchanged, and reject that edit before creating a fork. Keep normal first-message
editing, Claude, and Grok behavior intact. This task runs in Discussion Mode with
one implementation agent and parent review. The parent approved the final code and
explicitly authorized the local Carlos-only install. Do not commit.

## Progress

- [x] Reproduce the current bug with installed Codex 0.167.0 and a localhost model.
- [x] Inspect exact-tag native TUI source and verify native runtime behavior.
- [x] Parent approves exact-item guard and Codex-only presentation design.
- [x] Implement and test live/history user metadata, guarded forks, and labels.
- [x] Run release/Codex/native-Claude checks and compare release performance.
- [x] Stage the package; parent engineering review PASS.
- [x] Install Carlos as a regular executable, preserve the helper, and pass installed checks.

## Surprises & Discoveries

- Multiple distinct human items can share one native turn. Carlos previously
  passed only that turn ID when forking, which removed earlier messages and tools.
- The real reported incident cut 21 minutes of a shared turn. The original thread
  remained intact; its most recent compaction was much earlier than that cut.
- Native 0.167.0 TUI rejects later steers before calling `revert_thread`, while
  ordinary initial-prompt edits still work. Older native versions used a different
  rollback path, so earlier experiences can differ.

## Decision Log

- Keep Role::User and use a separate UserSteer message kind for display only.
- Enable the native user ledger only for Codex. Preserve that choice when loading
  replacement history and after every fork. Count all native user items, including
  invisible empty inputs; deduplicate started/completed events by item and turn IDs.
- Before forking, page authoritative items in the selected turn, verify the exact
  selected item and original text, and count preceding user items. Do not guess from
  duplicate text or current busy state. Fall back to authoritative full history
  only when the server does not support the item-list method.
- Reject unknown, changed, or steered anchors without creating a fork or sending
  the edited draft. Do not rewrite private rollouts or retain UI context the model lost.
- Flow labels say `you (steer)`; Gutter adds a small non-copyable steer heading.
  Plain Claude/Grok user headings and grouping remain unchanged.
- Refusals show a concise transcript error and keep the edited composer. Carlos
  follows the bottom to make that error visible even when the selected prompt was
  far above the current tail. Existing earlier rows are not removed.
- Text with attachments/bindings that Carlos cannot faithfully restore fails
  safely before a fork instead of silently dropping those parts.

## Evidence and Validation

- Carlos reproduction: `/tmp/carlos-rewind-shared-turn-xf9s8lw7/`.
- Real incident: `/tmp/carlos-codex-audit-20261111/rewind-incident-evidence.json`.
- Exact native source: `rust-v0.167.0`, `app_backtrack.rs` lines 559–679 and
  tests 911–951; `app/event_dispatch.rs` lines 737–840.
- Native runtime refusal: `/tmp/carlos-native-tui-backtrack-p6ong9n2/`.
- Native ordinary edit: `/tmp/carlos-native-tui-ordinary-edit-5edyfmwh/`.
- `cargo test codex_ --quiet`: all 30 focused Rust cases passed, including
  missing/changed item identity, hidden earlier inputs, paged overlap, legacy
  item-method fallback, repeated text, original draft text, live event dedup,
  enabled-ledger reset/reload, and label/copy/count checks at widths 8–140.
- Five terminal cases passed: two new shared-turn cases, existing legacy,
  repeated-edit, and paged history cases. Artifacts:
  `/tmp/carlos-codex-steer-focused/` and
  `/tmp/carlos-codex-steer-focused-phone-fixed/`.
  The initial phone assertion expected a compacted tool's body to be visible;
  explicitly choosing `/tools full` then verified the retained body without
  changing existing phone tool folding. The first width-8 label check also found
  clipped wording; UserSteer now uses a short `❯ steer` title below 16 cells.
- Actual installed Codex 0.167.0 plus localhost fixed model and debug Carlos:
  `/tmp/carlos-codex-steer-native-focused/`. The test verifies live shared-turn
  labels, refusal before fork or another turn, preserved draft and source bytes,
  resumed labels, exact first-item edit with identical text, and unchanged source
  bytes after the successful fork. Three localhost model requests, no inference.
- Long native-format history with 160 humans and a native compaction checkpoint
  passed with the new guard: `/tmp/carlos-rewind-long-compact-l2g7o1dk/`.
  Source/fork pages contained 100+61 and 100+59 turns respectively; exact selected
  turn and retained latest-prefix output match. One localhost model request.
- Strict Clippy and full-file style checks passed during focused work.
- First full release run `target/ui-artifacts/run-DJPXUm` passed strict Clippy,
  501 Rust tests (one ignored), both 52-test helper runs (284 assertions each),
  and package smoke. Four of 120 terminal assertions then failed: three old
  theme/spacing checks did not expect the correct new steer heading, and one
  exact-byte check caught the fake peer's truncate/write window after a read call.
- The parent approved explicit new steer heading/row/color assertions while all
  old normal-user/tool/source-color assertions remain. The fake peer now saves a
  PID-specific sibling and atomically replaces its snapshot. The exact source-byte
  assertion remains, with no delay or weakened comparison.
- All four affected release terminal cases pass with those final fixtures at
  `/tmp/carlos-codex-steer-release-atomic-fixed/`. No product change was needed
  after the first release run.
- Final `bash scripts/test.sh --release --codex-contract --claude-contract`: PASS,
  artifacts `target/ui-artifacts/run-3RVa29`. Strict Clippy; 501 Rust tests with one
  ignored; 52 source and 52 standalone helper tests, 284 assertions each; package
  smoke; 120 terminal workflows; Codex contracts and four native Codex monitors;
  all four native Claude localhost cases passed. Formatting, full-file style,
  and whitespace checks also passed.
- Focused native evidence used Codex 0.167.0; the final runner observed
  `codex-cli 0.167.1`. No explicit Codex install or update command was run during
  this task. The final actual installed checks also used 0.167.1.
- Five alternating sequential release comparisons against the prior installed
  Carlos passed. Same 1000-turn synthetic input, seed 1, 24 tool lines, 160x48:
  median before/after replay 48.17/47.57 ms, layout 14.88/14.73 ms, scroll p95
  0.81/0.81 ms, typing p95 0.62/0.63 ms. Both render 39347 lines; no material
  regression. Runs, script, baseline hash, and results:
  `/tmp/carlos-codex-steer-perf/`.

## Reviewed package and installed checks

The parent reviewed the production guard, user ledger, presentation/count/copy
paths, focused Rust/terminal/native regressions, final fixture corrections,
release evidence, performance, and package hashes. Verdict: PASS, no findings.
The parent explicitly authorized a regular-file Carlos-only install.

- Reviewed package: `target/ui-artifacts/run-3RVa29/package/`; 409 file checksums
  verified, 410 files including the checksum list.
- Carlos SHA256: `d2f32fc5e89c96ad9f8cdd974505eb9ce0ec93adf2432d3d5964befbc8071a86`.
- Helper SHA256: `a44f4b3e02aeeac1f314eabd1c8ad1aac06b291a204fbd342d35944ac7c7311a`.
  The packaged helper exactly matches the existing installed helper. The installer
  did not replace it; bytes, inode 3822776, mtime, and atime remain unchanged.
- Durable package/notices/rebuild files:
  `~/.local/lib/carlos/releases/0.1.0-codex-steer-d2f32fc5/`.
- Previous Carlos backup, SHA256 starting `230bffc1`:
  `~/.local/lib/carlos/backups/carlos-before-codex-steer-20261111T051712Z-230bffc1`.
  Its adjacent `-install.json` preserves the prior install metadata.
- Installed `~/.local/bin/carlos` is a regular executable, mode 0755. The installer
  staged a regular file, verified its hash, and atomically replaced Carlos.
  Native executables, logins, user settings, and previous backups were not changed.
- Installed `--help` and three focused workflows passed with zero failures/errors:
  actual native Codex shared-turn steer refusal/draft/source preservation, resumed
  labels and normal first-item edit, plus repeated-text and phone/prefix tests.
- `/proc` checks prove every test used the actual installed Carlos path, with
  native 0.167.1 in the native case. No helper override or copied test binary.
  Every home/config was isolated and every model reply came from a localhost
  fixture. No paid inference or real-session mutation.
- Installed smoke: `/tmp/carlos-codex-steer-installed-m00kj49x/`;
  driver `/tmp/carlos-codex-steer-installed-smoke.py`.
- Durable install metadata: `~/.local/lib/carlos/current-install.json` and
  `~/.local/lib/carlos/install-history/codex-steer-20261111T051712Z.json`.
  Review/install evidence: `target/ui-artifacts/run-3RVa29/{handoff,install}.json`.

All model tests use local fixtures. No real sessions were resumed or modified.

## Outcomes & Retrospective

Complete: Carlos distinguishes Codex turn-starting user rows from steers and
rejects an independent steer edit before creating a fork. It keeps the edited
draft and prior conversation, including source tools and replies. The exact
normal first user item remains editable even when later steers exist. Unknown or
changed native identities fail before mutation, and labels never enter copied
prompt text or model input. Claude and Grok behavior remains unchanged. No commit
was made.
