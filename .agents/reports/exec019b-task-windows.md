# EP019b: independent older task pages

The daemon can now serve different older Codex task pages to two terminals. Each terminal keeps its own page, selected task, detail text and draft. In F2, `l` reads the next older native page and `r` returns to the latest page. A failed read leaves the accepted page visible. Commits `acabde6` and `fea7b89` contain this work.

The daemon binds each page handle to the socket and session that opened it. A close, detach or socket EOF releases that reader's data. An unfinished native read still occupies its original slot until its reply or twenty-second deadline, so repeated open/close requests cannot accumulate uncounted native work. The terminal waits on one deferred read rather than polling with a new timer.

The latest and older task metadata share the existing eight-MiB allowance and eight native windows. If the latest metadata grows, core retires enough older caches to retain the authoritative latest facts. The terminal can keep its previously accepted display, but an expired page cannot authorize a stop. This allowance counts retained encoded metadata; it does not bound total process memory to eight MiB. Existing reply, parser, cursor and ancestry bounds still apply separately.

Before stopping an older process, core reads that captured owner's current process pages and checks the original process id, item and PID. A replacement rejects the original stop. Owner-specific native item events retire already queued proofs, including events without a process id. Unrelated owners do not retire them. Agent stops retain their original native turn and read id. No list read resumes a child or sends a prompt.

The final checks passed 1,396 Rust tests, strict workspace Clippy, four new daemon cases, 21 F2 terminal workflows including six new older-page cases, and two Claude panel cases. The worker also built both staged commits in separate checkouts without the unfinished settings editor. Raw evidence is `/tmp/carlos-task-windows-handoff.md`; final terminal artifacts are `/tmp/carlos-task-windows-ui-final-s1iocfj5`. One rejected run reused an artifact directory and mixed old request logs with new requests; a fresh directory passed without a product change.

Child activity, history and questions remain the next task checkpoint. The final integrated EP gate, embedded-path removal and whole-EP reviews remain open. The owner forbids intermediate reviewers, so none ran for this work. Main, the installed executable and remote branches remain unchanged.
