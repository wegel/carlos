# Independent child readers in the terminal

Commit `ebbf99bfe5838030500c848950fb2ff7ca91a706` adds child history over the real daemon socket. Enter in the Codex F2 panel opens the selected task. Each terminal keeps its own reader, page, scroll and composer. The l key requests older items, r returns to latest, Esc returns to the task list and F2 closes the panel. Child output never enters the root transcript. Structured questions remain a separate, unfinished EP019b change.

The daemon issues opaque reader and item identifiers through task/activity/open, read, page, advance and close. A read that names the current change waits on its original admitted request until that child changes or closes. No terminal polling timer was added. Reader rights include the peer, root, backend and native connection. Closing one reader releases only that reader; stale replies cannot reopen it.

The native cache permits eight interests in total, including retired inflight requests, under one shared eight-MiB encoded-byte allowance. Each native item request uses limit 1; an explicit older batch covers20 items, up to128 items per interest. Existing20 second native deadlines remain. Display scalars are bounded to32 KiB, labels to4096 bytes and encoded reply pages to384 KiB. A terminal retains128 KiB of display text plus bounded labels and a16 KiB live tail. These limits count retained encoded bytes; parser trees, one exported record/page and codec buffers also consume temporary memory. They are not an RSS guarantee.

The embedded and socket panels share the child painter and core display formatter. Differential tests compare text, command and pretty-JSON output at UTF-8 cutoffs. An accepted page stays visible through read failure or a held request. After reconnect, stale rights require an explicit Enter on a revalidated task; the terminal does not silently select another child or reopen one.

The developer recorded these checks:

- `cargo test --workspace`:1431 passed and10 ignored, `/tmp/carlos-task-activity-workspace.log`.
- Strict workspace/all-target Clippy passed in `/tmp/carlos-task-activity-clippy.log` before later unrelated settings edits.
- Nine focused Rust cases passed, covering the common memory/interest limits, independent readers, malformed pages, source separation and checked client replies.
- Four actual-daemon cases passed in1.863 seconds, including held/out-of-order replies, UTF-8 paging, native EOF and independent live tails. Log: `/tmp/carlos-task-activity-native.log`.
- Thirty-one actual terminal F2 cases passed in23.021 seconds, including six new child-reader cases. They assert screens and exact native thread, limit, cursor and reader/window requests. Log: `/tmp/carlos-task-activity-ui-all.log`.
- The normal commit hook passed cargo fmt and cargo test. Changed Rust files stayed within400 lines and functions within60 lines.

The detailed developer handoff is `/tmp/carlos-task-activity-handoff.md`. The primary agent still owes the frozen integrated EP gate. No reviewer, EP approval, install, merge or push ran for this checkpoint.
