# EP019b local socket views and clipboard

The client-backed terminal now shares the existing /tools parser, pointer gestures, page-scroll rule, selected-cell clamp, clipboard extractor and status separator with the embedded caller. It loads saved device themes/output mode at its startup boundary. Display commands work offline and never create a native or daemon mutation. The two callers keep their own editor and viewport choices.

An actual socket workflow folds/unfolds a 456-KiB ANSI body, resizes to 28columns, checks its original red tail, selects and copies exact Unicode, scrolls with PageUp/PageDown and wheel events, then folds offline. The recorder confirms no session/send. Another copies the whole 552-KiB original Markdown source, including references and fences, through CtrlY after phone resize. No per-draw body join or core Message copy is introduced. The first phone check exposed a clipped reconnect action and missing separator; F5 reconnects now comes first, and both callers share the separator painter. Strict lint also removed an unused old AppState wrapper and one needless borrow.

The restored gate passes 49 daemon workflows, 191 terminal workflows (eight via the actual daemon socket), 67 helper and 15 architecture checks at target/ui-artifacts/run-xB3tHI. Workspace tests, strict Clippy and the 400/60 size check pass. Source logs: /tmp/carlos-ep019b/socket-navigation-{tests,ui-separator,restored-clippy,final-terminal}.log. Matching release builds and paired perf checks also pass; no installed binary changed.

## Engineering review

Reviewed the diff against `a3a7d4c`, including the new gesture helpers, socket navigation, local commands, resize anchors, clipboard paths, and terminal tests.

Findings: No `BLOCKER`, `MAJOR`, or `MINOR` issues found. The extracted helpers preserve the previous pointer behavior. Ctrl+Y borrows complete source text, and pointer copy uses the existing wrapped-row and Unicode extractor.

Corrective guidance: None required for this checkpoint.

I relied on the supplied test results and checked the code directly; I did not rerun the full gate. History paging and source eviction remain outside this checkpoint.

Verdict: **PASS**

The supplied first prompt incorrectly called both targeted workflows passed: the Markdown test had passed, while offline folding revealed the missing phone separator. The scope was corrected, that painter was shared and both actual tests then passed. The reviewer checked the added painter/footer changes in a narrow follow-up:

Reviewed the shared separator painter, remote footer split, reconnect hint, and supplied terminal-test log.

Findings: No BLOCKER, MAJOR, or MINOR issues found. `render_status.rs` preserves the original separator width, color, row, and draw order. The footer split preserves composer, approval, and cursor behavior.

Corrective guidance: None required.

Both targeted tests passed in 1.133 seconds. Full checks and strict lint remain pending; this verdict covers only the requested changes.

Verdict: **PASS**

## Next consumer proof

The second Fable 5.2 medium whole-roadmap review is .agents/reports/exec019b-fable-source-cache-review.md. Codex accepts its work order. A real 200-tool native history probe reproduces the common-case gap: the initial snapshot has 100 metadata rows but only 32 source watches; Home shows source 168, source 100 is absent, and no history request occurs. Evidence: /tmp/carlos-ep019b/many-source-negative.log and target/ui-artifacts/ep019b-many-source-negative. Keep counts/plans by stable entry and format, show loading state, evict/retry sources, and save the viewport anchor while the viewport is current before applying history. Then measure above-cache bodies before choosing a pinned source, file/mmap or within-block checkpoints. No file fallback or streaming Markdown rewrite has been implemented or accepted. EP019b remains incomplete and unapproved.


## Matched release evidence

Both candidate source versions were built explicitly with cargo build --release: a3a7d4c in /tmp/carlos-ep019b/pre-local-source and the local-view source in the active worktree. The baseline is /tmp/carlos-ep019b/pre-local-release-carlos,21,757,640bytes,SHA256167dcc89541535fb908c8fa37ae1d1da12a283cae7b45c08174f72fd23b271cf. The current executable is /tmp/carlos-ep019b/local-navigation-carlos,21,768,792bytes,SHA256869292a589a3d048c36af480821145f7ae10b2419e62a6cc4c52aef5fad1517f. A clean rebuild of the five active workspace packages restored target/release/carlos to that same current hash after the shared-target worktree build left the primary executable pointing at the other source. Dependency and Whisper artifacts were kept. No further baseline build should use that shared target directory.

The original release (c880c65c...) and EP019a release (f03799c8...) were compared alongside these two. Fifteen paired rounds rotated the four executables on CPU6 at160x48 with private XDG/config homes. Each ran carlos perf-session with the1000-turn/seed1/tool-lines24 synthetic session and the stale_transcript_replay.jsonl fixture. The harness sampled /proc/PID/status VmHWM every2ms only after /proc/PID/exe matched the tested executable; it does not use the launcher's inherited wait4 floor or the unavailable built-in RSS field. Raw logs, exact identities and summaries: /tmp/carlos-ep019b/socket-navigation-latency/ and its adjacent .py script.

Synthetic counts match at6001 messages,39347 rendered rows and6000 relevant items. Original/pre-local/current replay medians are47.36/47.28/46.89ms. Current scroll/typing/append p95 medians are.83/.62/.67ms, compared with.83/.62/.66ms before the local-view changes and.83/.61/.66ms for the original. The fixture keeps5/9/4 counts; current p95 is.44/.39/.46ms, compared with.44/.39/.47ms before this slice.

Current synthetic post-exec high water is27,488KiB versus27,328KiB before this slice,24,720KiB at EP019a and23,952KiB for the original. The fixture is15,140KiB versus15,080KiB before this slice. These figures include code/file residency and both still-linked UI paths; they are not heap-only measurements. The local-view changes add160KiB in this sample, while the larger rise from EP019a predates them. The final EP must compare the actual complete socket path and total daemon/client memory after deleting the embedded path. These embedded perf-session figures do not replace that gate.

The earlier mixed-profile probe used a staged debug package plus an older release. It is retained under /tmp/carlos-ep019b/socket-navigation-latency-mixed-profile and excluded from all claims above. The first-consumer report now records the correction instead of silently changing its old hashes.
