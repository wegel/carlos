I fixed the redundant Codex request copy and made the completed-turn terminal checks wait for the completed screen. I worked in `/var/home/wegel/work/perso/carlos-daemon` on `daemon-split`, relative to HEAD `03ae85a929064a54ef36ef00814afae730dca23f`.

The Codex handler still rejects an answer unless its Carlos card id names a live card whose answer began while active. `SessionState::submitted_approval_answer` now returns borrowed immutable facts. `codex_reply::send` copies only `request_id` before calling the backend and updating the app. The same check still permits a validated form to finish after focus moves and rejects retired cards. I changed the generated test's comparison to borrow its expected facts too.

I checked the other answer paths against HEAD:

- Codex command, file-change, permission, legacy, question, and form replies enter `codex_reply::respond`. Its one `approval_answer` copy replaces HEAD's one pending-card clone. Both immediate replies and delayed form replies now pass through the borrowed submitted-card lookup. Error and retry paths add no full-request copy.
- Claude and Grok approvals, native questions, form answers, browser consent, and legacy exit-plan answers use `turn_approval::respond_to_pending_approval`. Its one draft copy replaces HEAD's one pending-card clone. `build_answer`, default-to-no checks, and `claude_browser::finish_answer` borrow that copy. Their response-building copies are unchanged from HEAD.
- `native_questions::draft_mut`, `codex_callbacks::edited`, and `codex_reply::interrupt` copy a native request id just as HEAD does. `codex_callbacks::checked` moves the validated candidate answer into the reply action; it adds no request-facts copy.
- I searched every production `approval_answer`, `submitted_approval_answer`, `to_draft`, and facts-clone caller and compared changed approval, question, callback, browser, and reply code with HEAD. The overlay renderer's separate draft copy already existed in HEAD. No other answer path added a full-request copy. `target/m4-fix5/answer-audit.diff` saves the source comparison.

I left the existing parsers unchanged. In particular, the Codex permission and legacy parsers retain their existing subsets of native payloads. The statements above concern stored request facts; they do not claim every parser stores every native field.

I added two cheap, deterministic allocation tests in `src/tests/approval_identity_tests.rs`. A test-only allocator counts calls and requested bytes on the current thread, only while the real reply handler runs. Setup, rendering, and concurrent tests do not enter that count. A scope guard disables the counter even when a test panics.

- `command_replies_copy_large_request_facts_only_once_for_each_backend` compares empty metadata with 100,000 bytes of stored metadata for Accept, Decline, and Cancel on Codex, Claude, and Grok. Each large request adds exactly one allocation and 100,000 bytes. The test also checks that the backend received an answer.
- `validated_form_completion_does_not_copy_request_facts` submits a real Codex form, finishes its worker check, and sends the resulting action through the real dispatcher. Empty and 100,000-byte stored metadata produce identical allocation counts and byte counts during completion. The test checks the exact backend answer.
- I deliberately changed `send` to clone the complete borrowed facts before extracting the id. Both tests failed. The command case copied 200,000 payload bytes instead of 100,000, and form completion gained one allocation of 100,000 bytes. I restored the source before building the final harnesses or running the required checks. Evidence: `allocation-tests.log` and `allocation-mutant.log` under `target/m4-fix5/`.

The terminal failure was a test race, option (b). I built HEAD in `/tmp/carlos-m4-fix5-head`, a detached worktree belonging to an independent clone at `/tmp/carlos-m4-fix5-repo`. This kept the owner's checkout out of the build and avoided writing its shared Git worktree records. I drove the same unchanged workflow and key sequence through the current and HEAD executables before editing the test:

- Current tree, original test: 7 failures in 50 runs, zero errors.
- Fresh HEAD executable, original test: 10 failures in 50 runs, zero errors.
- Every failure changed only screen row 30, the status band. The saved `before` screen contained `Running sdk-sonnet ~32k/123k 26%`; the later screen contained `sdk-sonnet ~32k/123k 26%`.
- A separate 50-run control sent no selection key after the answer appeared. In 10 runs, `Running` disappeared while the terminal waited for the completed band. Only the status row changed.

The fake Claude peer sends assistant text before its result and idle frames. `terminal.contains("Answer: ...")` could therefore return while Carlos still showed native work as running. The Ctrl+L handler clears only local selection state and writes a local hint; `set_hint` does not change Claude activity or turn facts. The HEAD comparison and no-key control establish the cause without changing runtime code.

The corrected test waits until the entire status band equals the fixture's completed band before capturing `before`. It waits on the drawn screen and adds no sleep. The original full-screen equality assertion and backend-request assertions remain intact. I also checked both other workflows added during this milestone: the default-to-no workflow waits for visible callback/consent text and does not capture a completed screen; the helper-replacement workflow had the same answer-text-before-rewind window, so it now waits for the exact completed band before rewinding.

I moved the helper-replacement test from `test_claude_questions.py` to the already-touched `test_input_status.py` so both files remain below 400 lines. The shared workflow class still runs the same test and every existing assertion.

- Current executable, corrected selection test: 0 failures in 50 runs.
- HEAD executable, corrected selection test: 0 failures in 50 runs.
- Current executable, helper-replacement test with completed-band wait: 0 failures in 50 runs.

`target/m4-fix5/` contains the repeat scripts, logs, JSON counts, before/after text screens, raw terminal bytes, keys, and backend traffic. The `before-repeats/` and `head-before-repeats/` directories each include `race-evidence.json`, which verifies the single changed row for every failure. `no-key-control.json` records the control runs.

I rebuilt both saved comparison harnesses from the final current source and fresh HEAD, then reran the complete saved inputs:

- All 470 consent cases and 2,406 checkpoints match HEAD exactly and also match the permanent fixture.
- All 24,300 mutation streams and 441,784 checkpoints match HEAD exactly, with zero panics on either tree. These include the original 24,000 streams and 300 local-event streams. The harness checks session facts, id relationships, transcript rows, redraw markers, backend calls, status text/bands, and approval-card cells/styles at both saved screen sizes.
- The harness rejects missing output and checks case/stream identities and record order. I restored every temporary source hook byte for byte before running the required checks. `source-hashes.json` records the final Rust sources.

Evidence: `consent-comparison.json`, `comparison.json`, `differences.json`, `parity-hashes.json`, `shards.json`, the harness sources/executables, and the progress logs under `target/m4-fix5/`.

I changed exactly six files this round, all already touched by the milestone:

- `src/session/approval_commands.rs`: 200 lines; borrow submitted facts.
- `src/app/codex_reply.rs`: 169 lines; copy only the native id in `send`.
- `src/tests/approval_identity_tests.rs`: 291 lines; add the allocator and two regressions.
- `src/tests/approval_operation_sequences.rs`: 385 lines; compare borrowed facts.
- `tests/ui/test_input_status.py`: 249 lines; wait for completed bands and house the helper-replacement test.
- `tests/ui/test_claude_questions.py`: 382 lines; move that test without dropping any checks.

The edited Rust functions remain below 60 lines. `round.diff`, `round-files.json`, `changed-paths.json`, `initial.diff`, and `final.diff` save the scope evidence. The complete Git path/status list is unchanged. No edit falls outside the milestone's already-touched source/test paths. I did not edit `.agents/` or `docs/`, touch the owner's checkout, commit, push, stash, reset, install, change the toolchain, or add a dependency. Normal builds and tests show only HEAD's existing deprecated `fetch_update` warning at `src/protocol_writer.rs:72`; I added no compiler warning. Temporary HEAD harness helpers emit their existing harness-only dead-code warnings.

I ran every required check on the final source. `checks.json` and the named logs under `target/m4-fix5/` record the commands, exit codes, and elapsed times:

- `build` (`cargo build`): passed. Total elapsed: 3.964 seconds. Log: `build.log`.
- `build-release` (`cargo build --release`): passed. Total elapsed: 51.658 seconds. Log: `build-release.log`.
- `fmt` (`cargo fmt --check`): passed. Total elapsed: 0.404 seconds. Log: `fmt.log`.
- `style` (`bash scripts/check-rust-style.sh --all`): passed. Total elapsed: 1.805 seconds. Log: `style.log`.
- `test-1` (`cargo test`): 855 passed, 3 ignored; tests finished in 3.80 seconds. Total elapsed: 17.615 seconds. Log: `test-1.log`.
- `test-2` (`cargo test`): 855 passed, 3 ignored; tests finished in 3.77 seconds. Total elapsed: 3.924 seconds. Log: `test-2.log`.
- `test-3` (`cargo test`): 855 passed, 3 ignored; tests finished in 3.77 seconds. Total elapsed: 3.941 seconds. Log: `test-3.log`.
- `terminal-1` (`bash scripts/test.sh`): 174/174 terminal workflows passed; 855 Rust tests passed with 3 ignored; UI tests took 78.752 seconds; artifacts: `/var/home/wegel/work/perso/carlos-daemon/target/ui-artifacts/run-USRMI5`. Total elapsed: 114.081 seconds. Log: `terminal-1.log`.
- `terminal-2` (`bash scripts/test.sh`): 174/174 terminal workflows passed; 855 Rust tests passed with 3 ignored; UI tests took 78.074 seconds; artifacts: `/var/home/wegel/work/perso/carlos-daemon/target/ui-artifacts/run-XqTReQ`. Total elapsed: 110.228 seconds. Log: `terminal-2.log`.
- `terminal-3` (`bash scripts/test.sh`): 174/174 terminal workflows passed; 855 Rust tests passed with 3 ignored; UI tests took 78.270 seconds; artifacts: `/var/home/wegel/work/perso/carlos-daemon/target/ui-artifacts/run-X6wgUk`. Total elapsed: 110.097 seconds. Log: `terminal-3.log`.

The three `cargo test` runs were consecutive. The three `bash scripts/test.sh` runs were also consecutive. Each full script also passed both 59-test Claude controller runs, the socket-recorder test, and the packaged-binary smoke check. The script staged its test package under the artifact directory; it installed no binary.
