# EP019b latest task terminal slice A3

The shared painter landed as 06e265d. The daemon-backed latest task panel is committed from 33 explicit owned paths; its commit ID is below. This is A3 only. Root explicitly deferred older native windows (B) and child activity/history/questions (C). The default embedded path, startup/settings/dictation and whole EP019b gates remain open. No install, main change, merge, push, Fable review or EP approval occurred.

The terminal opens F2 through the existing typed Client task subscription, current pages, selected details, named scalar reads and exact current-run/stop commands. Each terminal keeps its opaque selected task, bounded32-row page, complete displayed fields, scroll, Claude queue mode, composer and private stop results. Codex agent x refreshes the original current-run read before submitting one exact stop. Process PID/item/run/root/generation checks and Claude helper checks remain in the daemon. Unknown/native-reading/closed/not-ready states cannot authorize controls. Disconnect cancels unfinished x-read consent and preserves private pending/uncertain operations for explicit recovery, never replaying writes. Deleted/reordered tasks do not redirect a stop. F2 close reveals untouched underlying composer/editor.

One Client replica remains authoritative. Replica::task_sequence exposes the existing accepted task/full-snapshot sequence so an old failed read retires only after a newer task/snapshot fence, including A→readingB→A; root-only transcript deltas do not retry it. A single future page/header waits for the normal stream. TaskRevision includes the existing bounded run-read stamp, allowing same-value read completion to wake the original TaskReadId continuation. Fresh panel opens refresh latest once only when no native list is already pending; simultaneous opens share that read. Existing daemon interest/timer ownership remains unchanged.

Claude F3 stays private to the device and sends existing SendPolicy::Queue. The narrow engine wait exemption is Claude-only; ordinary FIFO/worker/command-idle/fork/rename barriers and the Codex queue-control overtaking guard remain. Actual held-turn test asserts native priority=later, original human UUID and F2 accepted-input overview. No new protocol method, server task scheduler, arbitrary native JSON route or settings/default mutation was added.

The shared pure frame uses text::pages from root prerequisite ce25616 and line helper d9c0740. It caches one sanitized body, original source map and exact compressed row ranges; scrolling paints only visible rows. A 4 MiB single ASCII token at one column uses one 16-byte row-run record. Root text.rs ASCII-cache hunk avoids rescanning shrinking suffixes. After the full gate, the unchanged hard-wrap loop was extracted to meet60lines; workspace/strictlint/build and real4MiB/phone checks passed again. The final shared event helper extraction only separates redraw choice/cache dirtiness to stay below60lines; the commit hook reruns workspace Rust.

Pending relative arrows retain original identity until the destination arrives; another revision drops only the cached destination. Explicit replacement choice after a proven missing row stays absolute. The task-only finite clamping transform preserves Up/Down order for every possible fresh count1..4352, including opposite keys at boundaries and changed counts. It stores17,440 bytes and measured2,000 keys in12.214ms (~6.1us/key). It stores no action queue/cap. Eight page tests plus exhaustive short sequence/count equivalence cover these rules.

# Accepted checks

- Full scripts/test.sh --codex-contract PASS: /tmp/carlos-task-terminal-final-gate.log; artifacts target/ui-artifacts/run-HAZxTM.1351Rust passed/9ignored;61actualdaemon34.303s;237terminal273.508s;23nativeCodex28.110s;67helper,15architecture,package,schema. Codex0.167.1.
- Strict final Clippy PASS: /tmp/carlos-task-terminal-final-clippy.log. Post-style workspace/clippy/build: /tmp/carlos-task-terminal-post-style-{tests,clippy,build}.log. Final commit Clippy/hook logs matching /tmp/carlos-task-terminal-commit*.
- All17new actual terminal/native workflows PASS12.697s: /tmp/carlos-task-terminal-final-ui.log, task-terminal-final. Includes two terminals,45row pages/deletion, exact agent/process/Claude stops, held/shared list, same-value run wake/original read, superseded x consent, unavailable subscription, pending/uncertain/private rejection/reconnect, root-only delta read fence,108KB Unicode command chunks,4MiB responsiveness and phone resize.
- Final full-gate4MiB scroll/F2:0.08858s at run-HAZxTM/ui/test_daemon_f2_four_mib_native_command_keeps_scroll_and_close_responsive/tasks-large-responsive/task-latency.json, versus10.445s before repair.
- Post-style4MiB+phone actual checks PASS2in1.847s: target/ui-artifacts/task-post-style.
-15focused task-local Rust tests PASS0.02s: /tmp/carlos-task-terminal-transform-tests.log. Tests compare deferred arrows with sequential keyboard semantics; cache tests independently compare the preserved wrapper over widths1..80, tabs, trailing spaces, blank rows, Unicode/ZWJ and long tokens.

# Reviews and negative proofs

- Initial A3 engineering FAIL2MAJOR: /tmp/carlos-task-terminal-review.md, raw session /tmp/carlos-task-terminal-review-session.log. Late native-run replacement hid private rejections;4MiB redraw blocked keys. Exact new test FAIL8.774s /tmp/carlos-task-terminal-review-negative.log (task-review-negative); large-field FAIL10.445s /tmp/carlos-task-terminal-review-large-negative.log (task-review-large-negative). Repaired actual pair PASS1.926s /tmp/carlos-task-terminal-review-fixed-ui.log (task-review-fixed). Follow-up PASS/no findings /tmp/carlos-task-terminal-review-followup.md.
- Deterministic task/list relay pause exposes lost Down during revision gap: /tmp/carlos-task-terminal-arrow-negative.log FAIL8.640s with saved old binary. Repaired pair PASS1.600s (task-arrow-fixed).
- Narrow arrow review FAIL1MAJOR /tmp/carlos-task-terminal-arrow-review.md: another revision before destination discarded pending movement. Exact pure test fails selected33 vs31 in /tmp/carlos-task-terminal-arrow-review-negative.log; repair follow-up PASS /tmp/carlos-task-terminal-arrow-followup.md.
- Intermediate full run-n195RK61daemon and235/236terminal passed; missing selected row's explicit replacement arrow failed across another revision. Exact pure negative /tmp/carlos-task-terminal-removed-arrow-negative.log. Final edge audit /tmp/carlos-task-terminal-final-audit.md found another Down could incorrectly convert pending absolute choice to relative missing identity. Private rejection during held metadata refresh was checked correct. Both arrow cases and per-key boundary clamping are now covered by finite transform. Final scoped review PASS/no findings /tmp/carlos-task-terminal-transform-review.md; reviewer independently ran updated Up→Down real gap case. Raw review session/prompt files use matching prefixes. All reported MAJORs are closed by these PASS follow-ups.
- Intermediate run-mENaaR61daemon and236/237terminal passed; phone test sent desktop resize before F2 close was acknowledged. keys.log showed buffered F2 delivered after resize. Test now waits actual task/unsubscribe before desktop resize, preserving screen/error/native/exact draft assertions; final full gate passes.

# Ownership and handoff

Historical owned39path manifest: /tmp/carlos-task-terminal-owned-paths.txt. Final A3 exact33stage manifest: /tmp/carlos-task-terminal-commit-paths.txt. Shared hooks are remote.rs, remote/events.rs, remote/render.rs, remote/drafts.rs; app/mod.rs and native legacy views belong to already committed painter. No edits to remote/bodies,reading,navigation,transcript or source/cache/actor/probe paths. Root owns their remaining working tree and reports/plan. Recorder additions are opt-in task-only one page error and explicit pause/release; defaults unchanged. New task UI mixins are registered in test_workflows.py. No native peer implementation file changed.

Root may reclaim all shared hooks/client/core/docs/recorder after final commit/handoff notice. A3 requires only existing typed routes and the narrow native read-stamp/fresh-open behavior above. This slice does not claim B/C or full task parity, and will not start them without root's later instruction.

Final A3 commit: 4f783494af4771e5559709f1ccfa78dd95027afc


## Raw review: carlos-task-terminal-review.md

Reviewed the frozen A3 diff and relevant callers. All 13 phase-A terminal tests and three embedded-panel checks passed. No source edits or Cargo runs.

1. **MAJOR: The panel hides stop failures after the task starts another run.**  
   [tasks_stop_results.rs:147](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/tasks_stop_results.rs:147) requires the notice’s run to match the displayed run. I held a stop acknowledgement, published `replacement-run`, then released a rejection. The terminal received `PRIVATE_TASK_REJECTION` but displayed no failure. Lines 137–141 also hide rejected notices for completed tasks.

   Keep pending and rejected results visible for their original task, labeling them as belonging to a prior run when needed. Add a terminal test covering this sequence and confirming that another terminal sees no private result.

2. **MAJOR: Large supported fields stall keyboard handling during redraw.**  
   [task_panel_view.rs:60](/var/home/wegel/work/perso/carlos-daemon/src/app/task_panel_view.rs:60) wraps the entire body on every redraw before selecting visible rows. The existing wrapping helper repeatedly scans the remaining suffix of long ASCII words.

   With the current debug binary at 36 columns, a valid 4 MiB command finished loading in 0.64 seconds, but a scroll followed by F2 took another 10.37 seconds. The equivalent 1 MiB case took 0.65 seconds.

   Use a linear wrapping pass, reuse wrapped rows while text and width remain unchanged, and keep scrolling work proportional to visible rows. Preserve complete field access and add a large-field responsiveness check.

Reproduction artifacts: [hidden rejection](/tmp/carlos-a3-review-stop-reject-m56a_6z8), [large-field pause](/tmp/carlos-a3-review-large-hhvuqw_d).

**Verdict: FAIL**

## Raw review: carlos-task-terminal-review-followup.md

Reviewed both MAJOR repairs, their concrete callers, cache rebuild hooks, and focused tests.

- Both previous findings are repaired: private stop rejections retain their original task/run, and cached scrolling copies only visible rows.
- No new BLOCKER, MAJOR, or MINOR findings within scope.
- No corrective changes required.

Both focused terminal tests passed against the current debug binaries. The 4 MiB scroll/F2 check took **0.093 seconds**. I also checked the supplied 39-test Rust result. No source edits, delegation, or Cargo runs.

**Verdict: PASS**

## Raw review: carlos-task-terminal-arrow-review.md

Reviewed the relative-arrow repair, three Rust tests, terminal test, and pause/release hooks. The focused terminal test passed against the current debug binaries.

- **MAJOR: A further revision can still discard a pending arrow.** In [tasks_pages.rs:166](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/tasks_pages.rs:166), `settle_target` replaces `Relative(delta)` with `Position(index)` before the destination page arrives. If another revision calls `reset` during that read, [reset:31](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/tasks_pages.rs:31) preserves only `Relative`, so it drops the move and retains the original selected task.

  Concrete case: use the reverse-page test with task 33 selected and delta −2. After scanning page two, the target becomes position 31 while task 33 remains selected. Reset to revision C before accepting the destination page, then scan C. The code settles on task 33 instead of task 31.

  Preserve the original task identity and pending delta until the code selects the destination row or proves the original task absent at EOF. Add a test that resets between locating the original task and reading the destination page.

No source edits or Cargo runs.

**Verdict: FAIL**

## Raw review: carlos-task-terminal-arrow-followup.md

Reviewed `tasks_pages.rs`, all six page tests, and callers that discard superseded page reads.

The reported MAJOR is resolved: the code retains the original selected task ID and pending delta until the destination arrives. A reset clears only the cached destination. If the original task is absent at EOF, the code retires the move and retains its unavailable ID.

- Findings: no BLOCKER, MAJOR, or MINOR issues found.
- Corrective guidance: no further changes required for this repair.
- Evidence: inspected the failing regression log and six-test passing log. No edits, Cargo runs, or builds performed. The integrated gate remains pending.

**Verdict: PASS**

## Raw review: carlos-task-terminal-transform-review.md

Reviewed the pending-arrow code and focused tests for identity, key order, bounds, overflow, and page changes.

- **Findings:** No BLOCKER, MAJOR, or MINOR issues found. The second-Down repair preserves the absolute target, and the finite transform preserves per-key clamping.
- **Corrective guidance:** No further changes required.
- **Evidence:** Supplied 15-test Rust log passes. The updated terminal gap test also passed against the current debug binaries. No source edits, delegation, builds, or Cargo runs.

**Verdict: PASS**