# EP019b task-stop engineering review

Reviewer prompt: `.agents/reviewers/engineering_reviewer.md`.
Two bounded non-interactive read-only Codex review sessions on 2026-11-14.
The first attempted read-only sandbox could not create its bwrap directory; it established
no code findings. Repeating with accessible tools and the same read-only review instruction
produced the two code findings below. Neither reviewer changed repository files.

## Initial review

Reviewed all tracked and untracked changes against `95d2d01`, including native guards, daemon/Core/Claude replies, and task invalidation hooks. The seven focused core tests passed.

1. **MAJOR: Claude stops block the session owner.**
   [task_stops.rs:57](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/engine/task_stops.rs:57) calls `work.dispatch()` directly. For Claude, that reaches `ControllerTransport::send()` and `InputWriter::write_guarded()`, which waits up to 500 ms for stdin. During that wait, the owner cannot serve reads, process approvals, or apply native events. This violates the explicit requirement that the owner never wait on native I/O.

   **Corrective guidance:** enqueue the guarded write without waiting, then report its result asynchronously. Preserve the eight-stop limit and writer-time identity check. Add a daemon test that stalls helper stdin and verifies that reads and controls remain responsive.

2. **MAJOR: An unconfirmed write becomes a definite rejection.**
   [task_stops.rs:68](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/engine/task_stops.rs:68) converts every dispatch error into `Rejected`. Claude’s writer timeout explicitly allows that the request may already have been accepted. A delayed write can therefore stop the native task while Carlos reports rejection; any later acknowledgement is ignored because Carlos never retained the pending stop.

   **Corrective guidance:** distinguish failures proven to occur before any bytes were written from failures after a write may have started. Report the latter as `Uncertain`. Add a test covering a delayed write whose native result cannot be confirmed.

**Verdict: FAIL**

## Corrective work

The owner now installs a private pending stop before submitting its write to the existing
four control workers. Native replies and local dispatch completions may arrive in either
order. An unconfirmed write stays uncertain; a native ACK wins over a late local error.
The new socket test suspends only its owned compiled helper, fills that helper’s stdin pipe
with valid JSON whitespace and proves another client can read before the 500-ms writer
timeout. Reverting worker dispatch makes that socket test fail. Reverting uncertain outcomes
makes the focused core test fail. Source was restored after each negative check.

## Narrow follow-up review

Reviewed the two fixes, worker-slot lifetimes, pending records, native ACK ordering, and the guards that prevent stale writes.

Both prior MAJOR findings are resolved safely:

- The owner sends native writes through the existing four control workers. It installs each pending record before dispatch and releases each worker slot on `TaskStopDispatch`, even if an ACK already removed the record.
- An unconfirmed write returns `Uncertain`. A late writer error cannot overwrite an earlier native ACK.
- Root and connection guards remain intact. Prepared commands retain the exact native turn or Claude helper run; process claims still check PID/item identity.

I reran the nine focused core tests; all passed. The supplied compiled-helper pipe-full test and negative proofs directly support both fixes.

No BLOCKER, MAJOR, or MINOR findings within this scope. No corrective changes requested. This verdict does not cover the roadmap or the still-running full gate.

**Verdict: PASS**
