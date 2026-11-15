# EP019b child-stop metadata refresh

Opening child activity releases the terminal's latest-list polling interest. After a successful child stop, native completion can mark the latest list unknown. The daemon previously had no reason to reread that list, so the activity panel could remain on “Reading agents and background processes…” indefinitely.

The owner now starts one existing bounded list refresh after a matching accepted Codex task-stop acknowledgement. It keeps the polling interest off, retains the last accepted rows while reading and uses the existing root/connection checks, queue and deadline. It does not change any terminal's selected page or draft. Rejected, uncertain, mismatched and retired-root acknowledgements start no read. A duplicate acknowledgement cannot queue another refresh.

## Before and after

- The unchanged actual-native workflow failed before the repair: `/tmp/carlos-ep019b/cutover-native-fact-fences.log`. Its native trace contained the correct child `turn/interrupt` and an accepted private operation, followed by unknown task facts and no later `thread/list`.
- The focused owner regression reproduced the missing refresh before the repair: `/tmp/carlos-task-stop-refresh-negative-02.log`, one expected failure and five passes. The first compile attempt also caught and corrected a test argument requiring an owned String; it is not the causal failure.
- All six focused owner tests pass after the four-line repair: `/tmp/carlos-task-stop-refresh-positive.log`. They cover hidden-list interest, one refresh, duplicate acknowledgement, preserved accepted rows, rejected/uncertain/mismatched results and a replaced root.
- The unchanged actual-native workflow `test_native_codex_f2_reads_child_without_resume_and_interrupts_its_exact_turn` passes in 1.309 seconds: `/tmp/carlos-task-stop-refresh-native.log`, artifacts `/tmp/carlos-ep019b/task-stop-refresh-native/`. It checks the exact child/turn native interrupt, subsequent metadata read, native completion, readonly activity read, retained composer, no child resume, one original root prompt and no blanket process cleanup.

## Binary identity and checks

The frozen matching pair is `/tmp/carlos-ep019b/task-stop-refresh-pair/`. Both executables report source `2746ecd31c3ba88cd92af3463d55dae9`. Its `identity.json` records all 2,104 source paths and SHA-256 values for the executables and helper. It includes the coherent combined worktree, including the separate fresh-thread history and owned-process-group prerequisites. It is not an isolated accepted prefix and does not flip the default launcher.

- Carlos SHA-256: `0dea983512b413767b07fe666f91563fd1efdf6e4b95c609afae3ca30be5efb7`.
- Carlosd SHA-256: `731714830e07681988b24f13494412f2e9fd9749e81eca2fc2db34169dd242f7`.
- Build log: `/tmp/carlos-task-stop-refresh-pair-build.log`.
- Workspace tests: `/tmp/carlos-task-stop-refresh-workspace.log`.
- Strict no-default, all-target workspace Clippy: `/tmp/carlos-task-stop-refresh-clippy-final.log`.
- Formatting and Rust 400/60 checks: `/tmp/carlos-task-stop-refresh-fmt-final.log` and `/tmp/carlos-task-stop-refresh-size.log`.
- Normal commit hook: `/tmp/carlos-task-stop-refresh-commit.log`.

This repair adds no protocol method, timer, polling interest or native target. It reuses the existing one-shot read machinery after the original guarded stop succeeds.
