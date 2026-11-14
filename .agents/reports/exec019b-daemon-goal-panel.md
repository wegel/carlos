# EP019b daemon-backed native goal panel

Sol 6.1 High implemented this independent slice on the owner's explicit request to use a developer worker where natural. Source commit is `d24366fe58121d92eb54994b51d3fb77c7fce3b1`, scoped subject `tui: preserve native goals through the daemon client`. Root kept transcript/source work separate and inspected the merged client, daemon timer and terminal hooks after handoff. EP019b remains incomplete.

The opt-in socket terminal now opens /goal or Alt+G, shares existing goal text/editor widgets with the embedded terminal, and supports independent drafts, budgets, pause/resume/clear, frozen revision confirmations and Ralph conflict checks. Each terminal owns its editor and private operation status; the shared Client holds the only accepted goal facts. Status-only changes and clear preserve an unsent objective draft. Uncertain writes unlock a retained draft without replay, and a later result cannot erase newer local edits.

A narrow Client::merge_goal wrapper uses existing sequence fences and fact budgets. Goal read lifetimes use goal/subscribe and goal/unsubscribe. The pure client can record intent offline and restore it after accepted session facts. The daemon aggregates live peer interests into one flag that drives the existing native30-second timer; closing/detaching/disconnecting one client does not stop another reader, while removing the last reader stops idle polling. No terminal polling clock or generic scheduler was added.

The real held-goal restart proof found that a changed daemon instance could present a lower sequence that the old Replica rejected. The client now drops only old-instance shared replicas, preserving private drafts/uncertain operations; same-instance fences remain. A wanted-session query prevents duplicate resubscriptions. Early Enter while a session is restoring preserves the composer and allocates/sends no operation; a later explicit Enter after fresh Ready facts sends once. The terminal displays availability while it is not Ready.

## Checks and review

The worker passed1270 Rust tests (five ignored), strict all-target Clippy and the full scripts/test.sh --codex-contract gate:49daemon,207terminal and23native-monitor workflows, installed Codex contracts and helper/architecture/package checks. Artifacts are target/ui-artifacts/run-lY0EoZ; the full log is /tmp/carlos-goal-slice-final-checks.log. The strengthened failed-admission retry proof passes separately at target/ui-artifacts/goal-failure-retry-final. Ten additional terminal workflows include the real90-second aggregate-reader lifetime proof and deliberate early Enter/restart behavior. Existing embedded goal workflows also pass with the shared widgets.

The first substantive engineering review returned FAIL for two MAJOR findings. Opening a closed panel offline had failed to retain desired read intent; a later admitted subscription failure had not cancelled the server-side reader. Both are fixed and tested against the actual daemon/relay. Cancellation retains its original subscribe serial, so an old failure cannot close a newer lifetime. Explicit r retries a read and clears only the read diagnostic, preserving editor errors. Complete verdicts follow.

### Initial engineering verdict

Reviewed the supplied goal-panel code, Client reconnect behavior, daemon reader lifetimes, and test evidence.

1. **MAJOR: Opening the panel offline never establishes a goal reader after reconnect.** In [goals.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/goals.rs), `open_goal` shows the panel, but `request_goal` returns before recording interest when no transport exists. After reconnect, the Client therefore has nothing to restore. Pressing `r` sends only `GoalRead`, so periodic reads remain disabled until the user closes and reopens the panel.

   Record the wanted goal reader while offline and subscribe after session facts return. Add a terminal test that opens a previously closed panel offline, reconnects, and verifies that periodic reads start.

2. **MAJOR: A failed subscribe reply can strand a daemon reader after the panel closes.** The daemon’s [goals.rs](/var/home/wegel/work/perso/carlos-daemon/crates/carlosd/src/manager/goals.rs) records interest when `queue_read` succeeds, before the owner returns its result. If that queued read fails, `GoalWatches::failed` in [goal_watches.rs](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-client/src/goal_watches.rs) forgets the active reader. A later `close_goal` then sends no unsubscribe, although the daemon still retains that connection’s interest.

   Either roll back daemon interest when the subscribed read fails, or send an idempotent unsubscribe when the Client handles that failure. Add a test where an admitted subscribe fails at the owner, then verify that closing the last panel leaves no reader behind.

**Verdict: FAIL**

### Final engineering verdict

Reviewed the scoped change against HEAD `95d6a73`, including all new modules and necessary caller context. Excluded the ExecPlan and transcript/source probes.

No `BLOCKER`, `MAJOR`, or `MINOR` findings. Both previous defects are fixed: offline opens retain read intent, and failed subscriptions trigger cleanup guarded by the original request number. Draft locks, frozen goal revisions, daemon replacement, and independent reader lifetimes appear correct.

Checks I ran passed:

- All 63 client tests.
- Five core goal tests.
- Scoped diff whitespace checks.

I reviewed the supplied terminal-test evidence; I did not rerun the full contract suite.

No corrective changes required.

**Verdict: PASS**

Another client's queued Ralph toggle is not exposed by SessionInfo; each terminal shows its own pending toggle and the shared daemon still enforces conflicts. This agreed limitation adds no server-wide pending projection. Main and the installed binary remain unchanged. No Fable review, EP approval or embedded-path deletion occurred in this slice.
