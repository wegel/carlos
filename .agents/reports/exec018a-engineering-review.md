# EP018a direct engineering review

Codex applied `.agents/reviewers/engineering_reviewer.md` directly. The owner requested no implementation subagents; this report does not claim a separate or independent reviewer. Fable 5.2 medium independently reviewed the starting roadmap, as recorded in `exec018a-fable-review.md`.

## Subject

The review covers the changes after accepted EP018 (`f42d870`), the rebased runtime through `a5a3554`, the revised whole-spike roadmap, client protocol and terminal inventory. The spike now includes main's `06f1e96` Escape-burst fix. Review checks include output cursor/parser state, body revisions, draft ownership across queues/helper switches, prepared command order and stale replies, package/compiler guards, terminal parity and executable performance.

## Findings and corrective guidance

- MAJOR, fixed: edit dispatch/bind/failure still matched text and human origin without the ClientOperation. Another client's identical text could claim a queued edit. Those paths now also compare the operation; a core regression test attempts dispatch, UUID binding and failure from a different client before the actual owner succeeds.
- MAJOR, fixed: an automatic send failure could return an unowned editor recovery. Only human failures return recoverable editor text; automatic failures retain their session notice. An unrelated native UUID no longer dismisses a failed edit. A correlated successful human send still dismisses that client's badge and preserves its retained drafts. Tests cover both rules.
- MINOR, fixed: the first incremental parser covered split C1 CSI but missed C1 OSC/DCS/ST and intermediate ESC sequences. A named control-state enum now covers those sequences, with a live one-character-at-a-time test. The cursor remains bounded and resumes without reading an old prefix.
- MAJOR, fixed during the worker prototype: native consumed/completed facts may arrive before a send reply. A checked completion now avoids duplicate echoes and cannot resurrect a completed turn. Tests delay the reply until those facts have already changed the owner.
- MAJOR, fixed during output proof: a transcript reset followed by an edit of a retained row could preserve an old output cursor. Revisions now change independently of the pending reset; retired ids fail explicitly. A completed cursor also restarts on an unexpected late append.

- MAJOR, fixed: the queue dispatcher could dequeue a human/automatic prompt while a prepared mutation held the command slot. It now leaves the queue intact until completion. The delayed-worker test proves the same queued human text remains available afterward.

The revised roadmap explicitly requires EP019 to bind work to native transport handles, separate the current receiver/normalizer fields, serve typed native controls and pass all existing terminal workflows before deleting the embedded path. EP018a has no daemon scheduler or remote command handlers; its immutable work values and headless tests do not claim that the current BackendClient is universally Sync. The remaining transport I/O and bounded selected-task cache are named migration requirements, not hidden permanent bypasses.

## Verdict

Source findings above are resolved. Final verdict remains pending release terminal/contract checks and quiet performance measurements. No independent review is claimed.
