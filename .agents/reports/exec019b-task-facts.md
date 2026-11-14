# EP019b: native task facts and read interests

The worker committed this checkpoint as `9a9f7b554d0979b375d79cd2dd82a1663827a142`.
It follows the typed task client checkpoint `77dcfc7` and the shared named-text range
repair `e9f6087`. It supplies the daemon/client reads needed by the terminal task panels.
It does not complete those panels, EP019b, or the default terminal cutover.

## What the code now does

The daemon publishes compact TaskFacts through snapshots and numbered `tasks_changed`
events. Every session subscriber receives those facts, including peers that did not
request native polling. The facts contain separate metadata and overview revisions,
counts, known/reading/closed flags and bounded diagnostics. The shared Client keeps one
accepted copy under its normal fact budget. It rejects stale or future task headers and
checks private pages against that copy. Devices keep selected ids, pages, details and
unsent answers separately.

A connection opens `task/subscribe` after its session subscription succeeds. The daemon
combines current task readers before running the existing three-second Codex list timer.
Closing a panel, detaching a session or disconnecting removes only that reader. The last
reader stops periodic list refresh. The Client restores wanted readers after the current
connection accepts session facts; closing offline cancels that restore. A failed older
subscribe cannot cancel a newer reader. The code adds no terminal polling clock, scheduler
framework or native method escape hatch.

The read-only overview exposes the existing Claude main activity, thinking count, quota
windows and accepted prompt labels/state/priority, plus verified Codex process diagnostics.
It omits accepted prompt source and thought bodies. Pages contain at most 32 rows;
previews contain at most two KiB; named label/message reads return at most 64 KiB. The
existing half-MiB read reservation bounds replies. Typed task fields also expose the
native labels, turn state, process owner/item and stop availability shown by existing
panels. The core treats those labels as display text; stops still require opaque task/run
addresses and the original root, connection, run/PID/item checks at native dispatch.

Claude's optional quota reader now uses the same pure prepare step in the embedded
wrapper and daemon. The daemon sends its five-minute read through existing bounded
native action workers. A full backlog or failed write retires only that optional read,
without stopping the session or retrying in a busy loop. The native receiver preserves
the original read number, root/helper scope and stream revision. Old replies cannot
replace a newer scope or newer native usage facts. The device still computes countdowns
from absolute timestamps; this checkpoint adds no settings command.

The core keeps native task bodies out of the root transcript. It reuses cached task
metadata and overview digests on root text changes. Child approval changes refresh only
the active count through the existing approval revision. A same-run turn-state change
updates the metadata revision without changing the exact run token used by a stop.

## Checks and artifacts

The final `bash scripts/test.sh --codex-contract` run passed:

- 1,319 Rust tests, with eight ignored: 841 main, 83 client, 359 core, 12 protocol,
  22 daemon unit and two binding tests. Seven ignored tests belong to main and one to core.
- 60 actual-daemon tests in 40.163 seconds.
- 220 terminal workflows in 264.911 seconds.
- 23 native Codex checks in 28.745 seconds, plus the installed Codex contracts.
- 67 helper tests, 15 architecture checks and package smoke/schema checks.

The run used Codex CLI 0.167.1. Artifacts are `target/ui-artifacts/run-7dAG72`;
the full log is `/tmp/carlos-task-facts-full-repeat.log`. Strict all-target workspace
Clippy passed in `/tmp/carlos-task-facts-clippy.log`. The commit hook ran the workspace
Rust tests successfully. All 74 committed files passed whitespace checks and the
400-line module/60-line function caps. Root's source/watch/cache files were stable for
this run and stayed out of this commit. Cargo.toml and Cargo.lock did not change.

Four focused daemon tests prove that two readers retain independent native refresh
lifetimes, the last close releases the timer, session detach rejects/releases work,
compact facts reach an observer without polling interest, and named task fields preserve
native display/process identity. They passed in 17.208 seconds at
`target/daemon-artifacts/task-interest-wired`. A separate native Claude overview test
passed in 0.473 seconds at `target/daemon-artifacts/task-overview-wired`; it checks actual
activity/quota rows, exact named labels, another reader's numbered revision, absent prompt
and thought source, and one daemon-scheduled `usage.optional` request.

Seven quota tests cover a held captured command endpoint with responsive owner reads,
a full 64-action backlog, dispatch/scope failure, the five-minute deadline and newer
native stream facts. A real controller receiver test checks the original quota read
identity in the returned context. Five core publication tests include 64 root text changes
with a large native task description, Unicode/EOF/revision checks, and callback arrival
and retirement without rebuilding metadata or overview digests.

The first interest run failed three of four tests because the actual socket method
whitelist omitted the new methods. The worker added those four names and reran the tests.
The first workspace run also caught the embedded quota test's old exact payload assertion;
the test now checks the new correlation keys while preserving all existing read/redraw
deadline assertions. An initial full gate then passed at `run-Lx8GU8` before the review
repairs: 1,310 Rust tests/seven ignored, 58 daemon, 219 terminal and 23 native checks.

## Review defects and repairs

The first engineering reviewer found two MAJOR defects and returned FAIL. The worker
added actual-daemon tests for both. With the repairs removed, both tests fail in
`/tmp/carlos-task-facts-review-negative.log` (two failures, 0.743 seconds), with artifacts
under `target/daemon-artifacts/task-review-negative`. The worker restored the repaired
source and binary, then reran them successfully in
`/tmp/carlos-task-facts-review-fixed.log` (two tests, 0.460 seconds), with artifacts under
`target/daemon-artifacts/task-review-fixed`.

The first defect left cached active counts unchanged when a child approval arrived or
retired. The core now checks the existing approval revision and refreshes only the count;
it preserves metadata/overview revisions and their hash counters. The second defect let
accepted turn state change while its task metadata revision stayed equal. The digest now
includes the accepted state. Both inProgress/completed transitions emit numbered task
facts, reject old detail/text revisions and retain the same opaque stop run token.

The follow-up reviewer returned PASS with no new findings and independently reran both
daemon regressions. Raw verdicts follow. The prompts and full session logs remain under
`/tmp/carlos-task-facts-review*`; the two verdict files are
`/tmp/carlos-task-facts-review.md` and `/tmp/carlos-task-facts-review-followup.md`.

### Initial engineering verdict

Reviewed the supplied diff within the owned paths, including task facts, read lifetimes, overview bounds, and quota guards. I reproduced two defects through the existing daemon binary without editing source or starting Cargo builds.

1. **MAJOR: Task counts miss child approval changes.**  
   [task_publication.rs:65](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/session/task_publication.rs:65) reuses cached counts when neither the metadata stamp nor dirty flags change. However, `task_counts()` also counts pending child callbacks, which neither mechanism tracks.

   With one idle child, I observed `counts.active` remain `0` after its approval arrived. A forced native refresh corrected it to `1`; resolving the approval then left it stuck at `1`. Subscribers without polling interest can retain these incorrect counts indefinitely.

   Refresh the cached counts when callbacks enter or leave, while preserving the cheap path for transcript text changes. Add tests for both callback arrival and retirement without a native list refresh.

2. **MAJOR: A run’s status can change without changing its metadata revision.**  
   [task_fields.rs:57](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-core/src/session/task_fields.rs:57) now reads `LastTurnState` from accepted run evidence. The metadata revision hashes the accepted run ID but omits its status.

   I refreshed the same native run from `inProgress` to `completed`. `task/detail` returned the new status using the old expected revision; both `TaskRevision` and the session sequence stayed unchanged. Other subscribers therefore received no task change.

   Include the accepted status in the metadata revision while preserving the exact run token used for stops. Add a test that verifies a same-run status change emits `TasksChanged` and rejects the old detail/text revision.

**Verdict: FAIL**
### Repair follow-up verdict

Reviewed the two repairs, their tests, and the callback callers needed to verify them.

- Previous **MAJOR 1 resolved:** child callback arrival and retirement refresh active counts through `approval.revision`, preserve both cached revisions, and avoid metadata and overview hashes.
- Previous **MAJOR 2 resolved:** `task_revision` hashes the accepted run status while preserving the stop token. Both status transitions publish numbered task events and reject stale detail/text reads.

No new **BLOCKER**, **MAJOR**, or **MINOR** findings. No corrective changes required.

I independently reran both daemon regressions: **2 passed**. The supplied core log shows **5 publication tests passed**, including the no-rehash check. I ran no Cargo builds.

**Verdict: PASS**
## Work still open

The socket terminal still needs the latest task list/detail/current-run/stop widget.
The next checkpoint will share the existing task painter and preserve F2/F3, narrow
screens, per-device selection/composer state, private stop outcomes and reconnect.
Claude F3 also needs the approved narrow repair that sends native priority=later during
a live turn while keeping ordinary FIFO/worker/fork/rename barriers.

Later checkpoints must keep independent older native list windows and child
activity/history readers, including two devices reading the same child. They must also
port the existing F2/F4 question/form editor and focus path with opaque card/field
identities and private answers. Browser/startup/settings/dictation workflows and the
separate embedded-path deletion gate remain open under the parent plan.

No binary was installed, merged or pushed. The worker ran no Fable review and approved
or archived no ExecPlan. The parent still owns the active plan and final EP019b review.
