# EP019b: native Codex queue panel through the daemon

The Sol 6.1 High worker committed the queue panel as
`c4bdbca7e4b6895d06bddf27f099977497b2bea2`. Root inspected its client merge/readiness checks,
ordinary-lane dispatch guard, private outcome handling and terminal controls after handoff.
This completes the native Codex queue slice, not EP019b or the default terminal cutover.

For Codex conversations, the terminal supports `/queue`, Alt+Q, F3, full-text editing, reorder/delete/start,
explicit refresh and failed/uncertain text recovery through the typed shared Client and real
daemon. It shares the existing drawing/caret helpers and owns no SessionState or BackendClient.
F3 adds a native queued message with its own private operation; it creates no ordinary chat echo.

The client keeps one compact QueueFacts copy under its existing byte budget. The terminal
keeps one page of 32 previews, at most 128 opaque continuation addresses and one complete selected
editor under the existing 16 MiB native text ceiling. Reads stay bounded at 64 KiB; submitted text
stays under the existing 512 KiB command bound. Selected opaque ids survive page/revision changes;
deleting or moving a target cannot silently select a different item. Rich native inputs stay
opaque with editing disabled. Each terminal keeps independent editor/composer/recovery rights.

A held chat-start reply prevented the old daemon from writing a native queue entry. The fix
uses the existing ordinary deque: only a QueueChange at its head can bypass a pending Prompt.
Fork/rename and earlier deque entries remain barriers. Existing bounded admission, queue
serialization, expected revisions and native root/generation guards still check the write.
Approvals and interrupts continue through their existing control workers.

A per-session accepted connection now separates a cached Ready replica from a successful
current subscription. Held, failed or superseded replies cannot authorize queue reads/writes
or ordinary sends. An empty successful replay redraws. Explicit F5 retries a failed subscription;
early Enter preserves text and allocates no operation. Goal watches also wait for the accepted
connection. F5 while hello is pending safely waits.

## Evidence

The final `bash scripts/test.sh --codex-contract` gate passes 1280 Rust tests, with six ignored;
53 actual-daemon tests, 219 terminal workflows including 12 new queue workflows, 23 native-monitor,
67 helper and 15 architecture checks. Installed Codex contracts and package smoke pass.
Strict all-target workspace Clippy and Rust formatting checks pass. Modules/functions stay
within 400 lines per module and 60 per function. Final artifacts: `target/ui-artifacts/run-cVnMvz`; full log:
`/tmp/carlos-queue-slice-final-pass.log`. Lint: `/tmp/carlos-queue-slice-clippy.log`.

Four daemon regressions prove native Add during held turn/start, queue-vs-queue serialization,
and held rename/fork barriers. Three deterministic terminal races prove stale continuation
recovery after another client's change from 45 to 46 entries, held same-daemon replay with an unchanged
editor revision, and failed resumed subscription followed by explicit F5. Both drafts and
private errors survive; no hidden read/write/send passes the failed subscription.
Focused artifacts: `target/daemon-artifacts/queue-dispatch-fixed`,
`target/ui-artifacts/queue-review-repairs-final` and `queue-relay-footer-fixed`.

The first full repair run exposed a test relay that treated a null reply as an object.
The relay now passes null, object, array, number and string results unchanged. The interrupted
run remains at `run-V3lQC5` with `/tmp/carlos-queue-slice-restored-final-checks.log`.
The final broad gate above uses the repaired relay and exact final queue source.

The worker returned every shared hook/client/engine/doc/recorder file after committing.
Root then replaced the private string-valued control selector with an exhaustive QueueControl
enum, removing its catch-all delete arm. It keeps the same explicit key-to-command mapping;
the repeatable terminal gate passes after that small follow-up: 1281 Rust tests, seven ignored, all 53 daemon/219 terminal workflows, helper/architecture checks and package smoke. Artifacts: `target/ui-artifacts/run-ws9dsg`; log: `/tmp/carlos-ep019b/queue-typed-controls-checks.log`. Broader contracts remain covered by the worker's final gate above. Claude queue mode and other remaining terminal workflows are still open.

No queue protocol method, generic scheduler, native timer or host-default API was added.
No binary was installed, merged or pushed. No Fable review or EP approval occurred.

## Engineering reviews

Raw read-only verdicts follow. The worker fixed every reported issue before the final PASS.

### Initial review

Reviewed the listed changes against `eda44e6`, including untracked queue modules and necessary callers. No source files changed.

1. **MAJOR: A stale page failure stops automatic queue refresh.**  
   [queue_reads.rs:103](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/queue_reads.rs:103) checks `reads.failed` before checking the latest queue revision. If another client changes the queue before the daemon handles a next-page request, the rejected read sets this flag permanently. New queue facts cannot restart the scan.

   Reproduced with 45 entries: another client added an entry before the next-page request reached the daemon. The terminal remained at “Refreshing: 46 queued” and sent no further reads.

   **Fix:** Track which revision failed. When newer numbered facts arrive, clear the obsolete failure and restart the scan while retaining the selected id and private editor. Add a deterministic terminal test for this race.

2. **MINOR: Reconnect reads can use the previous connection’s Ready state.**  
   [queue_reads.rs:88](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/queue_reads.rs:88) checks the cached replica without requiring the new subscription to finish. On reconnect to the same daemon, that replica can still say Ready while the daemon has changed state.

   Reproduced by withholding the resumed `session/subscribe` reply: the terminal sent `queue/read` before receiving fresh session facts.

   **Fix:** Require the current connection’s subscription to complete and confirm Ready before issuing queue reads. Add a reconnect test with the subscription reply withheld.

The four client queue tests passed. Two additional terminal/daemon probes reproduced the findings above; artifacts are under `target/ui-artifacts/queue-review-race` and `queue-review-resume`.

**Verdict: FAIL**

### Repair follow-up

Reviewed both repairs and necessary callers. The queue now resumes reads after a stale-page failure, and the terminal blocks reads and sends until the current Subscribe succeeds. Request matching, empty replay redraws, and existing goal reconnect behavior look correct. All 67 client tests passed; I also inspected the three terminal race artifacts.

- **MINOR: The footer retains the obsolete page error after recovery.** [events.rs:70](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/events.rs:70) copies the page failure into `status_hint`, but [queue_reads.rs:48](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/queue_reads.rs:48) retires only `Reads.failure`. The stale-page artifact confirms that, after recovering to 46 entries and closing the panel, the footer still says “queue changed or is unknown; refresh before submitting this draft.”

  Keep matched page failures out of the generic footer, or track their origin so retiring that failure clears only its footer copy. Extend the terminal test to check the footer after closing the recovered panel while retaining the composer and private editor error.

No BLOCKER or MAJOR findings remain within this follow-up. No files edited.

**Verdict: PASS WITH ISSUES**

### Final follow-up

Reviewed the named Rust functions, stale-page test, relay changes, and saved terminal traces.

I found no **BLOCKER**, **MAJOR**, or **MINOR** issues:

- Queue handlers consume only failures with matching pending request IDs; unrelated failures still reach the generic footer.
- Page recovery preserves private errors. The recovered screen shows 46 entries and the intact composer, without the obsolete footer error.
- F5 ignores sockets awaiting hello.
- A direct relay check passed null, snapshot, array, number, and string results through unchanged.

No corrective changes are needed. I did not rerun the full contract suite or Clippy.

**Verdict: PASS**
