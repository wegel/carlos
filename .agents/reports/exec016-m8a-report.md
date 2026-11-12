# Milestone 8.1: native queues and local editors

The session owns complete queue entries, authoritative/stale state, bounded page readers, revisions, uncertain native operations, submitted recovery text, backend errors and acknowledged notices. The view owns mode, visibility, selection, the editor and its original input, local edit errors and recovery text already offered to the user. Session functions return a small QueueUpdate; the app applies it once. The app no longer writes native queue fields.

## Rules preserved

- The shared native request counter moved into SessionState. Every existing controller still uses the same saturated sequence. Epoch/enabled/closed values remain in the old owner until milestone 9 and are passed to queue commands as plain values; no second stored copy exists.
- Tick still requires enabled, open, unknown, no active reader and no current error. A read has one 20-second deadline, 16 MiB/4096-entry limits, exact request token/revision and bounded unique cursors. Native pages still request one record. Stale reads discard their temporary page list and never replace the complete entries.
- Replies still check root thread, epoch, token, method and exact request params. Add/update/delete acknowledgments retain their old identity checks. Successful update clears only the queue editor; failed/uncertain operations offer submitted text once and never retry it.
- Local selection stays pinned to its native id. A client can request an edit only when known entries still contain the exact id/original input. Plain text editing still rejects attachments, spans and unknown input fields. Reorder/start/delete use only the supplied current selected id; the session builds their native order/params.
- Local errors take the same visible precedence as the former single error field. Every backend error replacement/clear reports that the view should discard its previous local error. Open and begin-edit clear the native error at the same points. The session retains backend facts when the view reports an unrelated local edit failure.
- On closure, submitted recovery text leaves its operation once; the reader is discarded and the queue becomes unknown. A thread switch resets queue facts and the old editor while preserving local queue mode. Reconnect installs the already-validated native queue and keeps the same selection behavior.
- Carlos automatic work still needs an authoritative empty native queue with no uncertain write and an open backend. Recovery/uncertain-write guards still prevent an edited-history switch from losing human text. No submitted message enters the transcript until Codex emits a consumed user item.

## Checks and direct review

All 993 Rust tests pass, including existing queue identity, stale-edit, attachment, epoch, deadline, EOF and recovery cases. A new test drives a complete queue read and a failed write with SessionState alone, asserts exact native params, proves automatic-work precedence and checks one-time recovery. All 174 terminal workflows pass in 82.575 seconds. Release build, format, all-file style, whitespace and function/file size checks pass. The existing protocol_writer atomic rename is the only warning.

The primary agent reviewed the moved guards, ignored replies, failure paths, bounds, identity checks and error precedence directly against `3bc2974` using the engineering reviewer checklist. Verdict: PASS, no BLOCKER or MAJOR. No subagent ran.

The 3,000-stream parent/pre-plan comparison and speed figures are recorded in the ExecPlan, with artifacts under `/tmp/carlos-m8a/`. This inherited corpus exercises transcript, callbacks, approvals and history for three backends; it contains no queue operation. The exact queue requests, stale-operation orders and visible queue workflows are covered by the enumerated Rust and terminal tests above, not by that corpus. No queue-specific mutation coverage is claimed.

The few additional caller paths only change queue view/fact paths or use the shared request counter: mouse/render paths, thread switch, reconnect install, panel/task/settings readers and their fixtures. No protocol method, dependency, schema or installed binary changed.
