# Carlos terminal workflow flakes

I repaired test races and a bug in the Python terminal emulator. I found no Carlos production defect in the three failing workflows. The patch changes seven files under `tests/ui/` and wires the new unit tests into `scripts/test.sh`; Carlos and the compiled Claude controller remain unchanged.

Final proof is complete.

## Measurements

- Goal editor: `test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts`.
- Claude rejected interrupt: `test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Codex reconnect drafts: `test_codex_reconnect_keeps_drafts_uses_current_fork_and_replays_full_items_without_duplicates`.
- `533fa12` is the original head of `daemon-split`; `94ba37f` is before its last three commits; `a59221c` is before the plan.
- The Claude workflow is absent at `a59221c`, so its pre-plan count is N/A, not zero failures.
- Every valid comparison uses Rust 1.100.0. Each loaded repeat ran while the release build was still active.

| Workflow | Commit | Conditions | Runs | Failures |
|---|---|---|---:|---:|
| Claude rejected interrupt | 533fa12 | Quiet | 150 | 1 |
| Goal editor | 533fa12 | Quiet | 150 | 0 |
| Claude rejected interrupt | 533fa12 | Release build | 50 | 1 |
| Goal editor | 533fa12 | Release build | 50 | 1 |
| Goal editor | a59221c | Quiet | 150 | 2 |
| Goal editor | a59221c | Release build | 50 | 1 |
| Claude rejected interrupt | 94ba37f | Quiet | 150 | 2 |
| Goal editor | 94ba37f | Quiet | 150 | 2 |
| Claude rejected interrupt | 94ba37f | Release build | 50 | 3 |
| Goal editor | 94ba37f | Release build | 50 | 0 |
| Codex reconnect drafts | 533fa12 | Quiet | 150 | 0 |
| Codex reconnect drafts | 533fa12 | Release build | 50 | 0 |
| Codex reconnect drafts | a59221c | Quiet | 150 | 0 |
| Codex reconnect drafts | a59221c | Release build | 50 | 0 |
| Codex reconnect drafts | 94ba37f | Quiet | 150 | 0 |
| Codex reconnect drafts | 94ba37f | Release build | 50 | 0 |
| Claude rejected interrupt | 533fa12 | Full suite before repair | 15 | 1 |
| Goal editor | 533fa12 | Full suite before repair | 15 | 0 |
| Codex reconnect drafts | 533fa12 | Full suite before repair | 15 | 0 |
| All 174 workflows | 533fa12 | Full suite before repair | 15 suites / 2610 cases | 1 suites / 1 cases |
| Claude rejected interrupt | 533fa12 + initial test fixes | First proof, original emulator | 20 | 0 |
| Goal editor | 533fa12 + initial test fixes | First proof, original emulator | 20 | 0 |
| Codex reconnect drafts | 533fa12 + initial test fixes | First proof, original emulator | 20 | 1 |
| All 174 workflows | 533fa12 + initial test fixes | First proof, original emulator | 20 suites / 3480 cases | 1 suites / 1 cases |
| Claude rejected interrupt | 533fa12 + tests.patch | Final quiet proof | 300 | 0 |
| Goal editor | 533fa12 + tests.patch | Final quiet proof | 300 | 0 |
| Codex reconnect drafts | 533fa12 + tests.patch | Final quiet proof | 300 | 0 |
| Claude rejected interrupt | 533fa12 + tests.patch | Final release-build proof | 100 | 0 |
| Goal editor | 533fa12 + tests.patch | Final release-build proof | 100 | 0 |
| Codex reconnect drafts | 533fa12 + tests.patch | Final release-build proof | 100 | 0 |
| Claude rejected interrupt | 533fa12 + tests.patch | Final full-suite proof | 20 | 0 |
| Goal editor | 533fa12 + tests.patch | Final full-suite proof | 20 | 0 |
| Codex reconnect drafts | 533fa12 + tests.patch | Final full-suite proof | 20 | 0 |
| All 174 workflows | 533fa12 + tests.patch | Final full-suite proof | 20 suites / 3480 cases | 0 suites / 0 cases |

The 15 baseline suites ran all 174 workflows each. Only the Claude rejected-interrupt workflow failed, once, with `AssertionError: 12 not less than 11`. Each of the other 173 workflows passed all 15 baseline suite runs. The first two workflows also failed in separate targeted repeats. The first 20-suite proof then exposed the reconnect workflow once: `IndexError: string index out of range` in pyte, followed by the same error during cleanup. That is one failed workflow case, with two recorded exceptions. The other 173 workflows passed all 20 runs of that first proof.

The exhaustive per-workflow counts, including every successful full-suite case, are in `/tmp/carlos-uiflake/measurements.csv`.

## Did the plan or the last three commits make either workflow fail more often?

The current tree had 1/200 goal failures and 2/200 Claude failures. The correctly rebuilt `94ba37f` had 2/200 goal failures and 5/200 Claude failures. The pre-plan commit had 3/200 goal failures. The current tree did not fail more often in these samples. These small samples do not establish equal long-run rates or prove that the last three commits improved them.

The goal test and its missing waits already exist at `a59221c`. `ac02f0a` added the Claude test, including its flaky order assumption, during the plan. The plan therefore added this flaky test; these failures do not show that it added a Carlos runtime defect. Neither named terminal test nor the compiled controller changed in the last three commits.

The reconnect workflow passed all 150 quiet and 50 loaded targeted repeats on each of the three commits. That sample did not reproduce its much rarer emulator failure. All three commits pin pyte 0.8.2, and the same minimal ANSI example crashes that emulator without Carlos. The runner has used `pyte.Screen` since `03f1d63`, before this plan. These facts do not support attributing the emulator failure to the plan or its last three commits.

## Failure messages

- Claude rejected interrupt, 533fa12, `head-alone`: 1 failure, `AssertionError: 12 not less than 11`.
  - Example: `/tmp/carlos-uiflake/head-alone/run-015/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Claude rejected interrupt, 533fa12, `head-load`: 1 failure, `StopIteration`.
  - Example: `/tmp/carlos-uiflake/head-load/run-002/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Goal editor, 533fa12, `head-load`: 1 failure, `AssertionError: Timed out: screen contains ' 修正'`.
  - Example: `/tmp/carlos-uiflake/head-load/run-003/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts`.
- Goal editor, a59221c, `preplan-alone`: 2 failures, `AssertionError: Timed out: screen contains ' 修正'`.
  - Example: `/tmp/carlos-uiflake/preplan-alone/run-087/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts`.
- Goal editor, a59221c, `preplan-load`: 1 failure, `AssertionError: Timed out: screen contains ' 修正'`.
  - Example: `/tmp/carlos-uiflake/preplan-load/run-026/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts`.
- Goal editor, 94ba37f, `before-three-rust100-alone`: 2 failures, `AssertionError: Timed out: screen contains ' 修正'`.
  - Example: `/tmp/carlos-uiflake/before-three-rust100-alone/run-015/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts`.
- Claude rejected interrupt, 94ba37f, `before-three-rust100-alone`: 2 failures, `AssertionError: 12 not less than 11`.
  - Example: `/tmp/carlos-uiflake/before-three-rust100-alone/run-093/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Claude rejected interrupt, 94ba37f, `before-three-rust100-load`: 3 failures, `AssertionError: 12 not less than 11`.
  - Example: `/tmp/carlos-uiflake/before-three-rust100-load/run-004/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Claude rejected interrupt, 533fa12, `head-suite`: 1 failure, `AssertionError: 12 not less than 11`.
  - Example: `/tmp/carlos-uiflake/head-suite/run-003/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker`.
- Codex reconnect drafts, 533fa12 + initial test fixes, `proof-suite`: 1 failure, `IndexError: string index out of range`.
  - Example: `/tmp/carlos-uiflake/proof-suite/run-019/test_codex_reconnect_keeps_drafts_uses_current_fork_and_replays_full_items_without_duplicates`.

## Diagnoses and repairs

The tests ran ahead of Carlos or assumed an order the SDK does not promise. Each repair changes the test or its fake peer.

### Goal editor: resize races the paste

The test sends `e`, pastes ` 修正`, then immediately calls `TIOCSWINSZ` to shrink the pseudo-terminal to 18 by 10. That system call changes the terminal size immediately. It does not wait behind the bytes in the terminal input queue. Carlos reads that current size before it dispatches input. `codex_goal_view::paste` deliberately consumes text without editing when the terminal is smaller than 22 by 12. A recorded paste before the recorded resize event therefore does not prove that Carlos handled the paste at the old size.

The earlier reported failure remains in `target/ui-artifacts/run-GxrdsH/ui/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts/`. Its keys include the paste, followed by resize, but the editor contains the original objective after growing again. The test never waited for Carlos to display the edit before shrinking.

The goal test now waits for the objective editor and the pasted suffix before shrinking. It also waits for the traced tiny-screen save key before growing, so the test really checks that Ctrl+S is ignored at the tiny size. All existing screen, draft, and outgoing request checks remain.

### Goal editor: reopen races the rejected pause reply

The head-load repeat 3 passed its first suffix check, submitted the edited objective, and received the expected rejection. It then sent a pause request. The test waited only until the fake backend recorded that request, then immediately sent `e`. `codex_goals::edit` correctly refuses to unlock the draft while the pause request is pending. When Carlos eventually handled the rejection, it remained on the overview. The test timed out at its second suffix check, at the original line 130.

The fake backend now names the field it rejects: objective for the edited save, status for the pause. The test waits for the distinct status rejection on screen before reopening. That screen proves Carlos handled the second reply and released the draft lock. The fake sends no new frames and changes no timings.

`git blame` attributes both missing waits and the immediate resize to `b029440`, before `a59221c`. The goal workflow is identical on all three measured commits.

### Goal editor: a stale title lets Escape and p become Alt+P

The first repair smoke run found one failure in five goal repeats. After the rejected save, the test sent Escape and waited for `Codex goal`. That title is present in the editor as well as the overview, so the check could succeed before Carlos handled Escape. The next byte was `p`. The terminal parser correctly interpreted the adjacent Escape and p bytes as Alt+P, which Carlos received while still editing the objective. The test then timed out waiting for a pause request it never actually sent.

`/tmp/carlos-uiflake/repair-smoke/run-001/test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts/keys.log` records `key code=Char('p') mods=KeyModifiers(ALT)` and no separate Escape. Its final screen still shows `Field 1/3: Objective` and the edited suffix. This is a test key-sequence race, not a lost key in Carlos. The repair waits for `Field 1/3` to disappear before sending p. The unchanged original test also had this race at `b029440`.

### Claude: native order differs from controller order

The fake native CLI executes one Python loop. For this mode it writes the successful result, the rejected interrupt control reply, then idle, from that same loop. The test should require this exact order on the native connection.

The compiled controller runs two asynchronous JavaScript paths in `helpers/claude-controller/src/controller.ts`: `Controller.readEvents` forwards the SDK event iterator, while `Controller.receive` awaits the interrupt promise and writes its reply. The SDK can reject that promise before its iterator yields a result already read from the native pipe. Consequently the controller may send either result/reply/idle or reply/result/idle without Carlos reordering anything. A single output writer preserves the order in which those two paths enqueue their frames; it cannot impose the native order between those paths. Carlos reads and traces that pipe serially in `src/claude_backend/transport_reader.rs`.

In `/tmp/carlos-uiflake/head-alone/run-015/test_claude_result_before_rejected_interrupt_finishes_without_a_stop_marker/`, `rpc.jsonl` has the native result at record 17 and rejected reply at 18. `claude-controller.jsonl` has rejected reply at record 17, result at 18, and idle at 19. The final screen shows `✓ Answer: hold this turn`, with no `Turn interrupted` marker. The failing assertion compared controller result index 12 with reply index 11. Carlos received the order that the controller actually wrote.

Under build load, repeat 2 instead read the controller log before the idle frame arrived and raised `StopIteration`. The native idle notification and completed answer do not prove that the controller's separate idle frame has reached its trace yet.

The Claude test preserves the existing controller order assertions and makes that schedule explicit. The test creates a named pipe, the fake CLI sends the result and blocks reading the pipe, and the test writes the first byte only after recording the forwarded controller result. The fake then sends the rejected reply and waits for the second byte. The test sends that byte only after recording the matching controller reply. Finally the fake sends idle, and the test waits until the controller forwards it before reading its log. These two recorded events force result/reply/idle on both connections without sleeps or longer timeouts. The real compiled controller and SDK still carry every message.

The test keeps the completed-answer check, both absence-of-stop-marker checks, the next human prompt, and its `later` priority. Carlos must complete the turn and accept the next ordinary prompt in either controller order. In `src/claude_backend/event_interrupt_pipe_tests.rs`, `result_rejected_reply_idle_uses_native_success` and `rejected_reply_result_idle_uses_native_success` deterministically send the two orders through real helper pipes; both require a completed turn. Each ran in all 20 final Cargo repeats. The terminal workflow `test_claude_failed_interrupt_rolls_back_intent_without_forging_a_stop` also keeps its opposite-order fixture and passed all final full suites.

`ac02f0a` added this terminal test and its controller-order assumption during the plan. `1ff03fd`, before the plan, introduced the controller's separate event and reply paths. None of the last three commits changed the two workflows or the controller.

### Codex reconnect: pyte crashes on an erased half of a wide character

The reconnect test failed once in the first 20 full-suite runs. The test had pasted `keep main 日本語` and opened the reconnect question. Pyte raised `IndexError: string index out of range` in `Screen.display`, where it reads `char[0]` for each cell. `Terminal.close` then raised the same exception while trying to snapshot that screen, leaving the child alive until the runner exited.

The captured bytes reproduce the problem without Carlos. Feeding them to pyte 0.8.2 leaves row 32, column 14 with `data=""`, left neighbor a space, and right neighbor `本`. The empty string is pyte's internal second-half marker for a wide glyph; Carlos does not emit that marker. Pyte replaces the first half of 日 with a space during the question redraw but leaves its second-half marker. Its renderer then indexes that empty string as an ordinary character.

A smaller fixed example is `日本語` followed by `ESC[1;1H` and `a`. Pyte leaves cells `a`, empty marker, `本`, empty marker, `語`, empty marker and crashes. This is valid terminal cursor movement and text; the terminal erases the other half when a glyph is overwritten. The error therefore belongs to the test emulator, not Carlos input handling or its backend traffic.

`TerminalScreen` renders orphaned halves as blank cells, keeps intact two-cell glyphs and their accents, and preserves color attributes in the underlying buffer. The tests feed every byte separately and assert the visible line and its cell width after each byte, including overwriting either half. A separate test makes snapshotting fail and checks that the terminal files still close and the original exception still reaches unittest. The reconnect test retains every screen and protocol assertion.

Evidence: `/tmp/carlos-uiflake/proof-suite/run-019/test_codex_reconnect_keeps_drafts_uses_current_fork_and_replays_full_items_without_duplicates/terminal.ansi`, `/tmp/carlos-uiflake/pyte-orphan-cells.json`, `/tmp/carlos-uiflake/reconnect-error.json`, and `/tmp/carlos-uiflake/reconnect-replayed-screen.txt`. All three measured commits pin pyte 0.8.2. No production fix or Rust regression test is needed for this emulator defect.


## Files repaired

- `tests/ui/test_codex_goals.py`: wait for the editor, pasted suffix, consumed tiny-screen save key, overview after Escape, and distinct rejected pause reply.
- `tests/ui/fake_codex_goals.py`: name the rejected field so the test can distinguish the objective reply from the pause reply.
- `tests/ui/test_claude_sdk.py`: create a named pipe and release each fake CLI step only after recording its matching controller frame; wait for forwarded idle before checking its log.
- `tests/ui/fake_claude.py`: hold the rejected reply until the test observes the result, then hold idle until the test observes the rejected reply.
- `tests/ui/terminal_screen.py`: render an overwritten half of a wide glyph as an erased cell, preserving intact glyphs, accents, colors, and screen width.
- `tests/ui/terminal.py`: use the corrected screen renderer; always close the child and terminal files even if a snapshot raises.
- `tests/ui/test_terminal_screen.py`: four deterministic regression tests for glyph overwrites, accents, colors, byte-split input, and cleanup; `scripts/test.sh` runs them.
- Patch: `/tmp/carlos-uiflake/tests.patch`. No production patches are needed.
- Every existing assertion remains. The tests still check the visible drafts, exact goal requests, lack of synthetic chat, successful turn, absence of a stop marker, and next prompt priority. The repair adds no sleeps, timeout increases, dependencies, or compiler warnings.
- The goal test failed 1/5 smoke repeats because Escape and p became Alt+P; Claude passed 5/5. After adding the overview wait, both passed 10/10 smoke repeats.

- Before the emulator failure appeared, both original workflows had already passed 300 quiet and 100 loaded repeats. After adding the emulator fix, all three passed another 300 quiet and 100 loaded repeats. I then added coverage for accents on wide glyphs, fixed that corner case, and started the final complete proof shown in the table. The earlier runs remain in their original artifact folders and are not counted as the final proof.

## How the repeats ran

The cheapest single-case command skips every build:

```bash
target/ui-venv/bin/python tests/ui/test_workflows.py \
  --binary target/debug/carlos --artifacts /tmp/unique-artifacts \
  Workflows.test_codex_goal_phone_resize_and_rejected_save_preserve_goal_and_main_drafts -q
```

`/tmp/carlos-uiflake/repeat.py` imports that same unittest module once, supplies the binary and a fresh artifact folder, then serially runs the original cases. Each case starts the real Carlos executable in a pseudo-terminal, a terminal supplied by the operating system, against the repository fake peers. The existing compiled Claude helper connects Carlos to the official SDK and the fake Claude CLI. The helper sources are unchanged across the compared commits, so each checkout used a copy of the same compiled helper.

The load command is `cargo build --release --target-dir /tmp/carlos-uiflake-load`. The runner records whether its build process is alive for every completed case, then waits for the build to succeed before starting another quiet phase. It cleaned only that temporary target directory before the final loaded proof to prevent a cached, already-finished build from counting as load.

The final load proof sets `CARGO_BUILD_JOBS=4` so the build remains active through all three workflows. The baseline and comparison builds use the default job count. Targeted quiet repeats run serially. The final full-suite proof runs two isolated Python processes concurrently, ten complete suites each, with separate artifact roots and fake backends; no build runs during those suites.

## Discarded compiler-mismatch trial

The first `94ba37f` trial used the system default Rust 1.93.0 after concurrent writes to Rustup directory overrides lost one entry. I discarded those counts as a comparison, recreated the worktree, set its override serially, verified `rustc 1.100.0`, rebuilt, and repeated all 150 quiet and 50 loaded runs. The tables above use only the corrected repeats.

The discarded trial remains under `before-three-alone` and `before-three-load`: goal 4/150 quiet and 5/50 loaded failures; Claude 6/150 quiet and 1/50 loaded failures. The corrected data use the `before-three-rust100-*` prefix. Compiler records are `/tmp/carlos-uiflake/before-three-rust100-toolchain.log`, `before-three-rust100-debug-rustc.json`, and `before-three-rust100-load-rustc.json`.

## Proof and cleanup

- `cargo test`: 20/20 runs finished; 0 failed commands. Logs: `/tmp/carlos-uiflake/rust/`.
- Each final Cargo run passed 890 Rust tests, failed zero, and kept the same three ignored tests. Across 20 runs: 17,800 passed tests and 60 unchanged ignored-test instances.
- `cargo fmt --check` and `git diff --check` passed. All changed Python files are at most 354 lines; every function is at most 55 lines.
- Carlos debug SHA-256 remains `21227c9bef3d4c0fe97e9c085762f5cdf68e4cbfdaf8d73513454703ecdc9a5c`.
- Compiled controller SHA-256 remains `d5a79722a6205cb5c185293165452330097106a9fef0345f5fffcb25422b4f2f`.
- The Rust compiler still reports the existing `protocol_writer.rs` deprecation of `fetch_update`. No Rust source changed and no new compiler warnings were added.
- The four terminal-screen unit tests passed; their log is `/tmp/carlos-uiflake/terminal-screen-tests.log`.
- Both temporary worktrees and their directory overrides were removed. The original branch and main-checkout toolchain are unchanged. No commits, pushes, stashes, resets, installs, `.agents/` edits, or `docs/` edits were made.
- Each phase retains `results.jsonl`, progress logs, terminal bytes, screens, keystrokes, native traffic, and controller traffic under `/tmp/carlos-uiflake/`.

## Unfinished

None.
