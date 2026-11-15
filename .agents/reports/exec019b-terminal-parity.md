# Check the existing terminal workflows through the daemon

EP019b remains incomplete. The terminal still has its old embedded route until
the complete daemon run passes and the separate deletion commit can remove it.
This report records checked repairs and the remaining failures, not approval.

## Preserve the same transcript painter

Commit `c8fa69a` removes the socket terminal's duplicate body/gutter painter and
calls the existing borrowed `TranscriptFrame` painter used by the offline
performance command. The owner publishes one optional stable entry identity for
its current reasoning step. It resolves that identity using its existing turn
boundary; the terminal never guesses from an old unfinished row.

The shared painter prefers a borrowed full input when available, preserving
first-line tool labels. It falls back to row facts for a large original whose
worker has already prepared painted rows. It decides whether reasoning fits
inline only from an available full input. This preserves prepared large bodies
without borrowing a core session or retaining another original.

A core test checks that an unfinished old reasoning row cannot become the new
turn's active step and that completion clears the identity. The exact six-path
index passed1540 Rust tests with10 ignored and strict no-default workspace
Clippy; the normal full commit hook also passed. Source tree and raw logs are
`/tmp/carlos-ep019b/active-reasoning-index-tree.txt`,
`active-reasoning-index-{tests,clippy}.log` and `active-reasoning-commit.log`.
The commit removes102 lines and adds58; it adds no command, worker or cache.

## Actual terminal checks

The first complete ordinary run through private test-owned daemons finished291
cases in837.807 seconds, with61 failures and one error. The complete log,
screens, keystrokes and both native/daemon traffic remain under
`/tmp/carlos-ep019b/history-ui-all-daemon-third*`. The numbers include repeated
cleanup failures and shared fixture assumptions; they are not61 independent
product bugs. The failed source-body test also mishandled a valid null
`saved/close` response while scanning unrelated replies.

The next root-owned group passes34 of42 cases in97.561 seconds against the
frozen matching dev pair `/tmp/carlos-remote-metrics-pair`. Its identity file
records terminal SHA256e513bf2d8293f218f75b594b42ad75185fc83dd3eaa68bfdca1ed67826730d90
and daemon SHA256412505272ba07a15d7507b43a1cecf97ef664c7ccceed16cf04106035ce0bc27.
This pair contains coherent concurrent work, not an isolated commit claim.
`history-ui-root-parity-fifth.log` records every case. Passing cases include
active reasoning glyphs/styles/spinners at narrow widths, tool folds, original
source colors, large ANSI source, Markdown tables, generated labels, rejected
subscription recovery and settings writes. A six-case follow-up passes the
context notice and completed-status checks; three reconnect cases and the theme
help remain open in that run. All failed logs stay beside the repaired evidence.

Two additional actual-daemon cases pass in1.100 seconds in
`history-ui-extra-sixth.log`. An empty prompt history preserves the private
composer and sends no chat/edit command. A fork/reset keeps the terminal's Flow
theme, unchanged local settings file and exact single native fork. These cover
the corresponding old AppState forwarding tests at the real client boundary.

## Repairs still being assembled

The terminal's outer F5 handler had bypassed its existing explicit subscription
retry. It now chooses a native Codex reconnect only for an accepted closed-owner
snapshot; otherwise it retries the rejected session subscription. The targeted
queue/source restore cases pass. A failed native Claude send reports Closed and
keeps its composer. F5 preserves a newer composer and explains how to recover
saved drafts; its private notice takes precedence over the generic saved-draft
badge.

The old appearance tests sent navigation as soon as the popup title appeared.
They now wait for the completed editor. New-session source/color/table tests
wait for an accepted ready snapshot before sending their first prompt. Startup
queue tests still deliberately type while Starting. A rejected settings editor
now closes explicitly before a test reopens its retained draft. Exact native
requests and screen/color/copy assertions remain.

The backend worker has committed same-owner explicit Claude reopen and is
checking current native identity and duplicate restore admission. The callback
worker is checking the remaining forms/tasks and accepted callbacks while
native Codex is Restoring. Root is checking mode notices, field help, Escape
timing and the final terminal matrix. A real native result-before-rejected-
interrupt case exposed another asynchronous lifecycle gap; that worker will
fix it with the original FIFO fixture. No reviewer, EP approval, install,
main change, merge or push occurred for these checkpoints.

The exact 4cf7912 source prefix passes 142 of143 daemon cases before the old queue test rejects every native caption, including the deliberately readonly display_id introduced in 3d0c65a. The corrected fixture asserts the two exact captions, opaque command IDs, and rejection of a caption as a command target without a native write. Its focused daemon test passes in0.438s. The full script is rerunning on the same source prefix plus this one recorded fixture correction; the manifest records that runtime sources did not change.

The old Escape tests exposed two real socket-client gaps. The actual Device tests fail for held Escape and composer paste before the repair and pass after it; the original quiet-interval timing oracle also passes. No timing window was loosened. Repeated Escape is ignored before dispatch, and ordinary paste clears both local Escape timestamps.

The frozen callback-complete matching pair runs all296 existing workflows through the daemon:294PASS/2FAIL in427.988s. Both failures are fixture observations, and the same pair passes both in1.464s after narrow fixes. The phone-process error now wraps across two lines after the truthful Stop failed prefix, so the fixture checks the exact complete phrase with screen whitespace normalized. The two-editor settings fixture previously accepted the original composer still visible behind an open settings popup as proof that the first Save had finished; its wire shows the second no-change Save arrived before the first native write. It now requires the first popup's footer to disappear before sending the stale second Save. All original native requests, default-file, selected-target and composer assertions remain. Logs: /tmp/carlos-ep019b/all-daemon-parity-complete-pair.log and all-daemon-two-fixture-fences.log; failed artifacts retained.

A new actual mobile-coordinate-paste case fails on that frozen pair: the daemon terminal lacks the old embedded mobile fallback. Preserve /tmp/carlos-ep019b/mobile-paste-before-final-fixture.log. The fallback is now an explicit remaining consumer gate, rather than removing its old AppState tests without a successor.
