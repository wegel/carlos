# Ralph controls through the daemon

Commit `acad30f` gives the terminal a typed `session/toggleRalph` command and optional
prompt-path/done/blocked/waiting choices. The daemon keeps the existing pending-toggle
flag. A second toggle cancels it. Once the native turn settles, the existing ordinary
worker reads the prompt file; the session owner can still handle reads and approvals.
Explicit `session/setRalph` remains an absolute on/off command. Stopping automatic work
retains accepted human input.

The terminal handles Ctrl+R after local editors. It requires current Ready facts before
allocating an operation, preserves the composer and displays the shared pending flag.
Private failures remain visible in the main status bar as well as the goal panel.
The next explicit attempt clears the previous failure hint. Reconnect never resends a
toggle. Prompt choices resolve against the daemon session's cwd and do not change defaults.

## Checked source and evidence

The isolated staged source is recorded in `/tmp/carlos-ep019b/ralph-index-tree.txt`.
Its checked pair and compiled Claude helper are frozen in
`/tmp/carlos-ep019b/ralph-final-pair/`. The source excludes the unfinished startup and
launcher work. This is a development-profile checkpoint, not the final release gate.

- `ralph-index-workspace-02.log`: 1500 Rust tests passed, ten ignored, including doctests.
- `ralph-index-clippy-02.log`: strict workspace/all-target Clippy passed.
- `ralph-final-daemon.log`: all 109 actual-daemon cases passed in 57.692 seconds.
- `ralph-final-ui.log`: ten Ralph-related terminal cases passed in 7.809 seconds;
  the two new cases use the real daemon. Older default-launch cases still use the
  embedded path until the complete legacy matrix switches to the daemon.
- The five new socket cases cover cancellation before a missing prompt is read,
  custom stop markers, private retry ownership, stopping with a queued human prompt,
  and FIFO-held prompt I/O while an approval completes.
- The two new terminal cases preserve a Unicode composer through deferred toggles and
  a held resubscribe. The held subscription allocates no Ralph request; a later explicit
  key works after the current snapshot arrives.

The first fixture runs failed because queue support was disabled and a marker appeared
inside a line. The fixed fixtures enable the existing queue and put markers on their own
lines. A later terminal check exposed the stale failure hint; the product fix passed the
same test. A production build also caught a test-only accessor; the runtime now uses the
unconditional accessor. The isolated source check caught two omitted test initializers;
both now supply absent options explicitly.

The inherited 61-line lifecycle function exceeds the repository cap in this exact prefix.
The next startup checkpoint fixes it; no Ralph function exceeds the cap. No reviewer ran,
no EP was approved, and no executable was installed. Full daemon terminal parity,
embedded deletion and final release/performance checks remain open.
