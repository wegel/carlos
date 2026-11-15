# Native process selection before Stop

The old `a_reused_process_id_with_a_different_native_item_never_retargets_stop` test required a fresh arrow selection when a native process ID referred to a new item. Its replacement now exercises the actual daemon terminal. The original embedded test and its single-use item setter are retired after the positive proof below.

## Failure and repair

The daemon previously hashed a process row's owner thread and process ID into its opaque TaskId. It hashed the native item and PID into TaskRun alone. After a list refresh, the terminal could retain the selected TaskId, read the replacement's new run and stop that replacement without a fresh arrow selection.

The new deterministic terminal case starts process `7` with `old-item`, selects it, then changes the same native process ID to `new-item`. It waits until the terminal displays the new item's complete details before pressing `x`. The old frozen executable sends `thread/backgroundTerminals/terminate` for the replacement. This is the causal negative in `/tmp/carlos-process-reuse-before-03.log`; the earlier attempt pressed Stop before details arrived and is not a product-failure proof.

The owner now includes the native item and PID when it hashes Process and WindowProcess addresses. A changed item or PID retires the selected address. A command-label change preserves it. Agent and Claude task addresses keep their existing rules. Existing root, connection, captured-window and exact-run checks still guard the native writer; this adds no command, state store or timer.

## Checks

- `task_view_tests::process_addresses_keep_metadata_but_retire_on_native_item_or_pid_replacement` checks stable label changes, separate item/PID changes, rejected old-address detail reads and rejected old-address run reads. The existing Unicode process test now requires a new opaque address after its item changes. All six task-view tests pass.
- All 65 Core `task_` tests pass, including older-window and writer-guard cases. Logs: `/tmp/carlos-process-identity-core.log` and `/tmp/carlos-process-identity-task-tests.log`.
- `Workflows.test_daemon_f2_reused_process_id_needs_fresh_selection_before_stopping` passes in 0.796 seconds. The old selection emits neither `task/stop` nor native terminate; the pane asks for arrows. An explicit Down selects a different opaque address, then `x` emits exactly one native terminate for the original owner thread and native process ID. The private main draft survives and no chat is sent. Log: `/tmp/carlos-process-identity-native-positive.log`; artifacts: `/tmp/carlos-ep019b/process-identity-after`.
- The combined-source workspace passes without default features: 1,569 tests pass and nine remain ignored. Strict all-target Clippy, the format check and the 400/60 style check pass. Logs: `/tmp/carlos-process-identity-workspace.log`, `/tmp/carlos-process-identity-clippy.log`, `/tmp/carlos-process-identity-fmt.log` and `/tmp/carlos-process-identity-size.log`.

The matching executables and helper are frozen at `/tmp/carlos-ep019b/process-identity-pair/identity.json`. Both Rust executables report source `9a66816a2ffa2e849eabe2479c1c73be`. This is a combined coherent primary worktree after `e1f5d0d`, with the process repair and green callback test drafts. It is not an isolated accepted prefix or the default cutover.

No reviewer, install, merge, push or EP approval ran for this bounded follow-up. Root owns the final joined terminal/native/release checks and the separate embedded-path deletion.
