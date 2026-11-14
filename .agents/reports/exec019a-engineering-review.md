# EP019a engineering review

Reviewed 2026-11-14 by the implementing agent, directly, against
`.agents/reviewers/engineering_reviewer.md`. This is a self-review, not an
independent review. The owner requested direct work without subagents.

## Subject

Reviewed the changes from planning base `68bc03c` through runtime source
`58e990100a708d322e6a113b683e42918c243846`: captured native command handles,
session owners, the local daemon, retry records, saved-session catalogue,
socket workers, protocol types and deterministic process tests. The branch
includes main `a29b1a1`, including its context-size controls.

The goal is a responsive, bounded server for multiple clients, with a later
terminal cutover and iPhone client. EP019a must preserve the embedded terminal
while proving the actual native adapters. It must not create a permanent second
prompt algorithm, a native-method passthrough or a transcript database.

## Findings and corrective guidance

No unresolved BLOCKER or MAJOR finding remains within EP019a's scope. The
following MAJOR findings were found during direct review and fixed before
acceptance:

- **MAJOR, resolved: queued native work could outlive its target.** Checking
  permission before calling an adapter did not protect a frame waiting in its
  writer queue. All three writers now check the captured connection,
  conversation and owner permission immediately before writing bytes. Reset
  retires unsent work without closing a healthy connection; possibly written
  work stays uncertain. Disabling the Codex dequeue check makes its real-writer
  barrier test fail (`/tmp/carlos-ep019a/m2-writer-before.log`). Preserve this
  check when EP019b adds native command families.
- **MAJOR, resolved: delayed file reads could change a replacement conversation.**
  Context/Ralph completions now verify their original session, native generation
  and conversation. Real Claude tests block a prompt-file pipe, reset the
  conversation, then release the read; the old read cannot enable Ralph.
  Shutdown cancels native work before waiting at most one second for workers.
  A remaining disk worker owns no session borrow.
- **MAJOR, resolved: an oversized shared page could hide private outcomes.**
  The owner returns shared changes as a fallible field alongside private results.
  A sync failure ends that interest visibly while preserving command recovery.
  The core update tests cover the oversized page and immediate read wakeup.
- **MAJOR, resolved: small read requests could retain large uncharged replies.**
  The manager now reserves 32 MiB for outstanding page/replay/output replies,
  in addition to request and socket budgets. Closing a peer releases its
  receivers and reserve. A real process test bursts 32 large history reads;
  raising the cap to one GiB makes its required capacity-error assertion fail
  (`read-budget-negative.log`). The restored limit passes. Keep reply bytes
  charged when adding new read types.
- **MAJOR, resolved: repeated stops could starve an approval.** Four interrupts
  for one turn could occupy all four control workers while the backend waited
  for an answer. The owner now claims one in-flight interrupt per exact turn
  before assigning work. An old completion cannot reopen another turn's claim.
  The real Codex negative case leaves the answer pending for eight seconds;
  the fixed case completes in 0.37 seconds (`stop-reserve-negative.log` and
  `stop-reserve-positive.log`).
- **MAJOR, resolved: Grok could retire its own cancel before writing it.**
  Grok now writes cancel/queue-clear outside its translator lock before emitting
  synthetic completion, and checks the captured prompt epoch at writer dispatch.
  A newly started prompt retires the old cancel without closing the connection.
  Reinstating the old order fails a real pipe/guard test before bytes are sent
  (`grok-order-negative.log`). A daemon process test also checks cancel arrives
  before the queued second human prompt.

**MINOR, accepted and measured: the terminal maps about two MiB more resident
file-backed pages.** Five alternating 4,000-turn samples show identical median
anonymous RSS (32,848 KiB); median file-backed RSS is 12,332 versus 10,332 KiB.
The full peak-RSS comparisons show a roughly constant difference at 1,000,
2,000 and 4,000 turns. These samples do not show extra transcript heap growth.
Keep the terminal cutover's memory comparison against these actual binaries.

## Boundaries and remaining gates

The daemon owns one manager and one mutable owner per live session. Native and
disk waits occur outside those owners. Workers use fixed captured handles;
short locks do not cover native reply waits. Shared event bytes are encoded once
per owner batch, while each client's output watches and command results remain
separate. The dependency checker rejects terminal/audio packages beneath core
or daemon and permits only serde packages beneath proto. Changed Rust files
remain at most 382 lines and functions at most 60 lines.

The embedded adapter is explicitly temporary, reuses native algorithms, and has
a separate deletion commit required in EP019b. The report-field inventory names
every deferred native panel family. Pages or plain rows above the eight-MiB
frame limit fail visibly; EP019b must provide bounded source reads and resolve
that limit before terminal cutover. Those are stated next-plan gates, not
claimed socket parity today. TCP, device pairing and actual iPhone code remain
EP020 through EP022.

## Evidence and verdict

Final default and no-default workspace tests, strict Clippy, release builds,
dependency/size checks, 16 daemon workflows, 183 terminal workflows, 23
native-monitor workflows and installed Codex contracts pass. The release runner
also runs 13 architecture tests and 61 Claude helper tests. Source/binary hashes,
exact counts, artifact paths and speed/memory methods are in
`exec019a-final-report.md`.

Thirty-one rotating paired terminal samples show no positive lower bound in
any 95% bootstrap interval for median slowdown against either preserved EP018a
or the original architecture reference. The standalone headless utility
separates source apply, export/encoding, writer and decoder costs; the actual
daemon separately measures 1,000 admissions through a deterministic native peer.
Neither claims to measure model latency or an exact worst-case heap bound.

Verdict: **PASS WITH ISSUES**. The only accepted current issue is the measured
fixed file-backed RSS increase. The owner must review the completed headless
step before EP019b begins.
