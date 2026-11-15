# EP019b final report

EP019b is complete on runtime source `23d6e70775c623b8106074957efb103d`, commit `cbdca8f70cf1e71461bba6985b5ed95d30e09e45`. The final release, terminal and installed-native checks pass. Quiet measurements use that same release pair. Root's direct engineering review is PASS; the independent Fable 5.2 medium whole-roadmap review is PASS WITH ISSUES, with every finding resolved or assigned to the required first EP020 milestone. Root records delegated approval in the completed ExecPlan before archiving it.

## What the user gains

An ordinary Carlos terminal now connects to the managed local `carlosd` process. The daemon owns native Codex, Claude and Grok sessions. Closing a terminal leaves its conversation running. Each terminal keeps its own composer, scroll position, selected task, open tools, display choices and failed-command recovery. Existing callbacks, tasks, queues, goals, settings, names, history edits, summaries and Ralph work through typed socket requests. Dictation stays local and optional.

The pure `carlos-client` crate checks greetings, numbered facts, history/source replies and private command outcomes. It owns no socket, clock, thread, terminal or native backend. Core and daemon dependencies remain free of terminal/audio packages. The terminal keeps the shared painters/widgets, original colors and exact source copy. One formatter worker holds its active original and renders requested rows. Client caches and transfer queues keep their bounds. Protocol version 1 remains provisional until real remote clients exercise it.

Commit `cc2a5f5` separately removes the embedded terminal runtime after actual socket parity: 110 paths, 8,503 deleted lines and 47 added lines. The executable no longer owns AppState, a mutable Core session or a native backend. Commit `30e053d` keeps the fixed-backend executor only in test builds. Native/Core/Device and pure widget tests replace the old mixed frontend tests. Two status sweeps move unchanged to their shared painter. Only two obsolete keyboard/native queue-order tests retire; ten actual Native pipe schedules and terminal checks preserve their useful behavior.

## Repairs and final review

The joined checks found concrete defects in edited-prompt echoes, reused native process IDs, native startup scope, owned descendant shutdown, queue/edit recovery and stale history navigation. The active plan and focused reports preserve their failing-before/passing-after proofs.

The final single-CPU socket stress caught Grok completion arriving before the write acknowledgement. Grok now returns its existing turn identity and uses one completion emitter for both native completion forms, including empty results. Four real-pipe cases preserve unsolicited-before-prompt behavior. The same repaired daemon admits exactly 1,000 native prompts without reviving a completed turn. See exec019b-grok-early-completion.md.

The large-source check caught a Crossterm resize/key wakeup defect. Selecting its existing alternate Unix reader fixes the lost key with one Cargo feature and one locked dependency. A controlled PTY test fails the old reader and passes eight fresh batches with the chosen reader. The original immediate resize→End gesture and deadlines remain. See exec019b-resize-input.md.

The final Fable review judged the architecture sound and found small residue at the client boundary. Commit `cbdca8f` removes Core F5/restart instructions, four unused TurnUpdate fields, obsolete focus effects and the captured reconnect editor. Panels use an explicit disconnect reason. F5 on a closed non-Codex helper preserves the draft and shows the actual remedy without resubscribing or sending. The extended Claude workflow fails the old release pair and passes the final source, including another terminal reopening the same session. The first correction stored a sticky hint; the full focused workflow caught it. The accepted code derives the hint from current availability instead.

The raw review is exec019b-fable-final-review.json; the narrow-screen text is exec019b-fable-final-review.md. Root's own inspection is honestly labeled in exec019b-engineering-review.md. exec019b-review-disposition.md records every finding and decision. No broad redesign, extra scheduler, timed merger or speculative phone command family follows from this review.

## Checks

The complete `final-release-gate-09.log` exits 0. Both workspace feature configurations, strict all-target lint, default release tests/build and 19 architecture/dependency checks pass. The compiled helper typecheck and 72-test suites pass. The same source passes 143 daemon cases in 72.816 s, 306 actual-daemon terminal workflows in 416.236 s, 23 installed Codex cases in 33.355 s and all 10 installed Claude 2.1.322 cases in 27.826 s. Selection (2), source-failure (5), dictation (2), ANSI-notice (1), native ownership and package smoke also pass.

A separate quiet run passes nine default-launcher cases and both actual 64 MiB ANSI/16 MiB Markdown workflows in 14.379 s. Those workflows preserve colors, immediate resize/Home/End, typing under source backpressure, reconnect and exact full original-plus-append copy, including the blocked clipboard case. The controlled PTY resize/key regression passes as part of the Rust checks.

The private test script relocates only artifacts to a fresh `/tmp` directory because this shell has uid 0 while the checkout sits below a uid 1000 home. It also supplies the absolute checkout and the existing failure-only diagnostics. Product path protections, assertions and deadlines remain unchanged. Installed CLI checks use private homes and local deterministic peers, without user sessions or paid inference.

Keep prior failures distinct from the final green run. Gate07 was interrupted after a focused test caught the first repair's sticky hint; gate08 was interrupted during its release rebuild to finish the retry reason and final subscription assertion. Neither is presented as a complete pass. The earlier source0c acceptance evidence remains separately identified and is not substituted for the final source.

## Exact release pair

The frozen executables and complete measurement-time file hashes are under `/tmp/carlos-ep019b/review-accepted-pair/identity.json`.

- Carlos: 17,334,632 bytes; SHA-256 `af9e3bb220fe4515f74adf8dc379874d3624478c8db14977864c9dd159777384`.
- Carlosd: 17,361,096 bytes; SHA-256 `2ecdbcd9be452ee835993dc5f410e310999a17de222c7f7e0386609b32c32551`.
- SDK helper: 104,780,090 bytes; SHA-256 `01a3485c9dabd395a0a763c58e49fcc22234cdd3d1081969d4266be8e0b7a42f`.

Carlos is about 15% smaller than EP019a's 20,346,176 bytes. Carlosd grows from 15,090,288 bytes as it gains the existing terminal controls. The helper's bytes/hash are unchanged. The final code audit finds at most 400 lines per Rust file and 60 per function, with no violations.

## Speed and memory

Every measurement below ran after builds, tests, hooks and workers finished. The renderer uses 15 alternating samples per binary/case, CPU 6, 160×48 cells, default release features, Rust 1.100/LLVM 23.1.5, thin LTO and one codegen unit. The synthetic case uses 1,000 turns, seed 1 and 24 tool lines. The captured fixture is byte-identical after its move to Core. Counts match across all three binaries: 6,001 messages/39,347 rows/6,000 items and 5/9/4 for the captured case. Kernel post-exec memory samples avoid the old perf-session RSS=0 limitation.

Values below list pre-EP016, EP019a and final EP019b in that order.

- Synthetic p95 scroll: 0.81/0.81/0.68 ms. Typing: 0.61/0.61/0.48 ms. Working: 0.61/0.61/0.48 ms. Append: 0.65/0.65/0.51 ms.
- Median cold replay: 46.26/45.49/47.61 ms. Full layout: 14.60/14.33/14.75 ms. Kernel peak RSS: 23,980/23,976/25,736 KiB. Paired replay cost versus EP019a: 2.06 ms, 95% interval 1.84–2.28. Paired RSS increase: 1,784 KiB. These are measured costs, not described as noise.
- Captured p95 scroll: 0.44/0.44/0.31 ms. Typing: 0.39/0.39/0.26 ms. Append: 0.45/0.46/0.32 ms. Replay: 0.97/0.99/0.77 ms. Peak RSS: 11,680/11,564/11,024 KiB.

The matched actual socket driver admits exactly 1,000 native Grok prompts in each daemon, with the same historical driver/fixture and unrestricted scheduling for both. EP019a median/p95/max: 0.161/0.203/0.799 ms. Current: 0.164/0.209/0.670 ms. After-admission RSS is 14,248 versus 17,440 KiB; anonymous memory is 5,280 versus 5,752 KiB. These figures include Python and native-fixture logging and measure admission, not model completion. The old reference can deadlock when every process shares one CPU, so that failed schedule is causal race evidence, not a latency sample. The repaired current pair additionally passes all 1,000 prompts entirely on CPU 6: median 0.505 ms, p95 0.647 ms, max 1.122 ms.

The owner/export replay transfers 1,060 frames, 1,000 chunks and 4,896,929 wire bytes. Core apply takes 13.86 ms, publish/export/encode 13.39, writer 5.77, decode 3.58 and overlapping transfer wall 25.58 ms. Kernel RSS goes from 2,680 to 7,944 KiB. This uses a real Unix pipe and production page/output types; it excludes native startup, manager admission and terminal paint.

The actual formatter/cache probes take 16 append samples and 16 resizes in each 28↔120 direction. ANSI initial layout takes 124.02 ms, append p95 0.74, resize p95 133.48/132.45 ms and final RSS 77,480 KiB. Markdown takes 38.05 ms initially, append p95 30.20, resize p95 21.03/22.18 ms and final RSS 106,096 KiB. Both retain at most 48 painted rows and 256 KiB of client-cache data. Row payload capacity peaks at 18,720/18,336 bytes; kernel RSS also includes spare vector slots and allocator overhead. Streaming digests verify exact original-plus-append bytes. These timings exclude socket, terminal and clipboard costs; the actual-source workflows test those behaviors separately.

Raw final evidence lives under `/tmp/carlos-ep019b/review-acceptance-evidence` and `review-matched-latency`. The committed exec019b-final-measurements.json keeps summaries and identities. Root accepts the small cold-copy/memory costs alongside the faster interactive path and smaller terminal binary. Typing and scrolling add no network round trip. No timed merger is justified by the admission figures.

## Limits and the next EP

The ordinary corpus does not bound every possible original. The earlier dense Unicode index still takes about 6 seconds, peaks near 1.3 GiB temporarily and retains about 238 MiB. Queue/cache bounds do not bound those arrays. The index runs in the terminal formatter, not the daemon or pure client. No body cap, file/mmap fallback or speculative phone renderer was introduced. EP022 must measure its actual phone renderer independently.

One historical Claude settings-reset timeout remains unexplained: the native peer sent reset/init/model response, but the trace does not establish a cause. Unchanged bounded probes and complete later gates pass at the original deadlines; no product repair is claimed. An earlier isolated Markdown-copy timeout also remains unattributed. Both original traces and the failure-only diagnostics are preserved. Any recurrence needs a cause, rather than acceptance after a rerun.

The daemon currently keeps live owners until shutdown and caps them at 16. The required first EP020 milestone will add explicit typed stop/reopen, free native processes/owner slots, preserve saved conversations and prove reuse before adding WebSocket/device-token access. Existing closed Claude sessions can already reopen through explicit saved-session startup. The new command must preserve private recovery and never replay uncertain text automatically.

Protected main remains `a29b1a1e3720e48b09e2ca984fb16c29b5857ea3`; installed Carlos remains SHA-256 `d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3`. No spike executable was installed, merged or pushed.
