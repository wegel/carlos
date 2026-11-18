# EP022 engineering review

Root reviewed the iPhone and terminal boundaries after the frozen simulator,
native backend and release gates. The daemon owns native facts. Rust checks
that each phone request still belongs to the current connection, session and
revision. Swift owns the saved servers, transport, drafts and view state. The
native list can defer updating visible cells during a gesture and restore a
stable anchor afterward. The terminal's fast ASCII wrap count uses the same
word records and optimal-fit choices as the painter on its supported subset.
The normal Unicode, ANSI and code paths retain the full separator.

## Findings

- **MAJOR, resolved.** The phone retained handled failures after the user had
  copied the text back to the composer. Textless control failures occupied
  the same slot budget. An explicit Restore or Discard now releases the exact
  operation after it handles the text. Failing controls settle their UI state
  and release the record. An accepted prompt stays as an optimistic echo in
  the current session until its authoritative row appears. When the user
  leaves that session, the confirmed send can release the phone's local echo.
- **MINOR, recorded.** The iPhone still rebuilds the list projection on some
  updates. The measured native motion and 10 MB source workflows passed. A
  future performance check can exercise thousands of loaded rows during live
  streaming. The independent review's remaining web boundary and mobile
  reliability notes are in `exec022-review-disposition.md`.

## Follow-up and verdict

**PASS.** The simulator bridge test rejected 129 controls and admitted the
next prompt without restarting the client. The corrected app's full matrix
passed 36 workflows and 27 unit tests on source 35f2cd499401ff683df5. Native
traces show that Restore preserves the exact text and sends nothing, Discard
preserves a separate draft, and the rename editor keeps its name until the
original reply. The independent end review was PASS WITH ISSUES. The code
and tests resolve its major finding. Root's follow-up checks the correction
without asking another reviewer to repeat the static pass.
