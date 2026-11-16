# EP021b end-review decisions

The independent review is `exec021b-fable-final-review.md`, session
`b6e45425-07a2-486f-9aeb-cd04cc495b99`, Fable5.2 medium, PASS WITH ISSUES and no
blockers. Its opening paragraph lists the files it did not read. Root's engineering
review and the concrete tests below cover those omitted areas. No second reviewer
run is needed for these corrections.

- M1, shared history reducers: accepted. Codex now restores full and prefix records
  through `restore_history_record` with an explicit Replace/Prepend mode. Prefix
  reports fill missing children newest first and cannot overwrite newer reports.
  Differential tests compare complete snapshots, tasks, tool facts, prompt choices,
  native rewind anchors and turn states. They normalize only owner-local IDs,
  sequence numbers and panel revision hashes. Claude uses the actual archive
  decoder for the older side and the full normalizer for the full side. Another
  test pins its deliberate exclusion of scratch live-turn, task, override and
  completion-guard bookkeeping; row completion and tool facts move with the
  transcript. The archive decoder emits no current task membership or turn lifecycle.
- M2, concurrent readers: accepted. Waiting envelopes retain the existing mailbox
  permit, original20-second deadline and root/helper target. The mailbox already
  bounds their count and bytes. Each owner pass checks retired/expired reads and
  accepted rows before dispatching I/O. Matching readers share an accepted native
  fetch. Cached readers and urgent controls remain independent. Codex and Claude
  actual-daemon tests, queued-reader stop and the actual terminal workflow pass.
  No wire command, retry policy or extra native reader was added.
- m1, nonempty native windows with no visible rows: accepted. A zero-row adoption
  continues under the same deadline and cumulative byte allowance. The failing
  original daemon test and corrected daemon/terminal tests cover unsupported
  middle items, exact native cursor progress and the oldest visible row.
  A follow-up boundary check also reproduces a newest page with no visible anchor.
  The native reader now uses the actual row projector to skip such windows before
  Ready, including empty user records. Directory-only formatting helpers avoid
  constructing another session or maintaining a second supported-type list.
  Separate overlap-only tests still exercise the owner's zero-row continuation.
- m2, failed cancellation leaves dead Grok cards: accepted. Mark choices cancelling
  and emit owner-card retirement before attempting the cancel write. Cancellation
  enqueue returns its captured tokens even on failure, so the owner still retires
  them. Never re-enable a grant after an uncertain write. Real broken-pipe and
  full-queue tests cover both paths. The actual terminal sees the card disappear,
  keeps its private draft and emits no grant or extra send.
- m3, reader waits on writer: accepted. Invalid-session, retired and unsupported
  callbacks enqueue bounded negative replies without waiting for the native pipe.
  The existing writer closes its owned process on an I/O failure, including jobs
  whose caller does not wait. A held-pipe test checks all three exact original IDs
  and reply bodies while proving the reader returns before the pipe is released.
  Existing queued-grant replacement/cancel tests remain intact.
- m4, display policy: keep native activity facts unchanged. The owner's requested
  Requesting suppression belongs to the terminal's display preference; the real
  footer flash came from replacing the whole status with source-read progress.
  Do not discard useful facts in Core to impose one UI's preference. Defer a numeric
  localizable line count until an actual consumer needs a protocol change; Swift
  must not parse the current English summary. Expand the existing table test to
  cover every fallback tool name, including semeja.
- m5, optional defensive-code trimming: declined. Existing malformed, stale,
  foreign, truncated, replaced-ID and cursor-loop tests justify these checks.
  Shipping the helper with the binary does not make pipe bytes or native replies
  reliable. Keep original budgets, deadlines and fingerprints.
- m6, retained older rows: accepted as a documented carried limit. The owner keeps
  loaded bodies as clients scroll back, within the existing caps. Eviction would
  need to preserve native identity, source and rewind semantics and is outside
  this phone-data plan. Keep it visible alongside the Claude SDK scan and Grok
  full replay, without introducing an unmeasured cache/controller here.

Focused checks pass. The complete corrected release/native gate, fresh matching
measurements and Root's final follow-up verdict remain pending; this document
does not yet approve EP021b.

Gate05 separately finds an intermittent Claude startup without context counts.
The unchanged isolated terminal rerun passes; the trace alone does not prove the
exact cause. A deterministic owner test reproduces a matching bug: a prepared
optional context read stays pending forever if capturing the native command handle
fails before dispatch. Clear that exact token through the existing failure path,
preserving known counts and the5-second backoff. The owner test fails before and
passes after this narrow fix. Keep the original terminal readiness assertions and
timeouts unchanged, and rerun the full gate. No inference, user session or extra
reviewer is involved.
