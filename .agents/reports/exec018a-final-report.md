# EP018a: pre-daemon cleanup

Final status: PASS, 2026-11-13. Release workflows, native contracts, feature checks and executable measurements passed.

## Result

A remote client can read and resume a running tool in bounded chunks. The core keeps one original body, preserving terminal colors and copy behavior. An opaque cursor carries the raw position and bounded terminal-control/diff parser state; a sparse body revision invalidates replacements. Each read consumes at most 64 KiB and emits at most 256 pieces. Compact output-change facts reach all session subscribers, while optional output watches use separate cursors. Collapsed clients receive no bulk body and skip no session sequence numbers. No socket server or watch scheduler is claimed by this step.

The core no longer receives ClaudeDraftContext, composer text, confirmation tokens or callback focus when it handles backend facts. Typed outcomes carry ClientOperation ids with submitted text through queues, edits and helper switches. Each client restores an empty composer or retains text beside newer typing, and ignores recovery owned by another client. Matching text alone cannot claim an edit. Automatic failures cannot populate a human editor. Shared notices now report recovered text without terminal key names; the terminal supplies F5 guidance. Notices deliberately remain visible even if one editor was empty or identical, so shared facts do not depend on a chosen client.

Send/steer, rewind/fork and rename use prepare, execute and finish methods. BackendWork owns no SessionState borrow and is consumed once. One outstanding mutation preserves command order; native/session/turn guards reject stale completions, and a reset cannot let an old completion clear a newer command. The queue retains its text until the slot is free. Delayed-worker tests prove reads and facts remain available, and cover native prompt consumption/completion before a reply without duplicate echoes or restarted turns.

`docs/terminal-client-migration.md` inventories local editors and every shared terminal workflow, including native tasks, queue/goals/settings, approvals/questions, names, rewind, quota, summaries, Ralph and dictation. EP019 must first prove a headless daemon with real native handles, add typed commands for that inventory, pass terminal parity and delete the embedded path separately. It cannot retain a permanent privileged bypass or generic backend-JSON command. Protocol version 1 stays provisional until real remote consumers exercise it. Bindings, generated schemas and timed delta mergers wait for concrete consumers or measurements.

The old 385-line source lexer and its 18 synthetic Rust fixtures are deleted. Cargo's resolved package guard still covers all features, platforms, aliases and dev/build edges. Real compiler checks protect private fields, opaque ids and foreign inherent impls. Review retains semantic editor/I/O rules that neither a token scan nor the compiler can prove.

## Source and checks

Runtime source: `43b34609eee2912077b7fbecb70de5025773036a`, checkout `/var/home/wegel/work/perso/carlos-daemon`, branch `iphone-spike`. During the work main advanced to `06f1e96137442aba5e7aea017fe40e5e8482f88c`; the spike was rebased onto that Escape-burst fix and retains its new terminal tests. The suppression timestamp belongs to ViewState. The main checkout was not changed by this work. No spike binary was installed, merged or pushed.

Compiler: Rust 1.100.0, LLVM 23.1.5. Installed Codex for isolated contracts: 0.167.1. Raw provenance: `/tmp/carlos-ep018a/accepted-source.json`.

The final default Rust tests pass: 816 application, 250 core and six protocol tests, plus three compiler checks; three larger audits remain explicitly ignored. Without default features, 780 application, 250 core and six protocol tests pass, with the same three compiler checks and two ignored audits. Strict Clippy passes with and without default features. The no-default release built successfully. All 11 architecture tests pass. Changed Rust files are at most 357 lines and functions at most 59 lines.

`bash scripts/test.sh --release --codex-contract` passed all 177 terminal workflows, 23 native-monitor workflows, installed Codex contracts, 59 SDK helper tests, recorder/screen checks and standalone package smoke. Artifacts: `target/ui-artifacts/run-rpOhMe`; log: `/tmp/carlos-ep018a/final-release-contract.log`. These checks used no inference or user session.

The accepted default executable is 20,188,600 bytes, SHA256 `3b7970cae7f4d33cc1687715ba6990e36315482324f8e015cc72fcff04c262be`. It adds 42,368 bytes (0.21%) versus accepted EP018, or 41,880 bytes versus EP018 rebuilt on the same main. The no-default executable is 18,053,048 bytes, SHA256 `083046dfa2a54428d4bce40328ece165da86502071365f34f67c15e576e5e1be`. The measured executable matches the release runner's binary.

## Executable measurements

Two quiet 31-round runs rotated the accepted executable, preserved EP018 and original pre-EP016 reference on CPU 6. Each executable used the same seed, 1,000 turns, 24 tool lines and 160 by 48 terminal; the captured stale-transcript fixture ran separately. All counts matched: 6,001 rows/39,347 lines/6,000 items for synthetic, 5/9/4 for the fixture. No build or UI test competed. The original reference had no metric with a positive 95% lower confidence bound in either run. Scroll, typing, working-draw and append p95 stayed at the baseline.

The old EP018 binary showed a repeatable roughly +0.05 ms full-draw difference; small replay differences varied between runs. The spike had also acquired main's Escape fix, so the review built a control at rebased EP018 `d9ff9018d26b24613b8d13f195098393607b2ac8` in a separate worktree/target. That control is 20,146,720 bytes, SHA256 `2681295242b1d950c6c9cc7fbbacabb3f9875fb5b1e97e29de24f3294b3b7e66`.

A fresh 61-round common-base comparison had no positive 95% lower confidence bound for any measured metric. Synthetic medians in milliseconds, rebased EP018/current:

- Apply: 14.20/14.20.
- Replay: 46.97/47.11.
- Full layout: 14.92/14.84.
- Full draw: 3.41/3.44.
- Scroll p95: 0.83/0.83.
- Typing p95: 0.62/0.62.
- Working draw p95: 0.62/0.62.
- Append p95: 0.67/0.67.

Paired apply changed +0.04 ms, bootstrap 95% interval [-0.04,+0.08]; replay +0.03 ms [-0.34,+0.47]; full draw +0.03 ms [0.00,+0.10]. Fixture replay medians both measured 0.98 ms. These bounds do not prove exactly zero cost; they establish no slowdown at this test's resolution against the common-base control, while the original architecture reference also passes. Raw samples and summaries: `/tmp/carlos-ep018a/latency/`, `latency-repeat/`, `latency-control/`.

Five alternating Linux wait4 pairs at each size gave EP018/current peak RSS medians of 25,668/24,372 KiB at 1,000 turns, 32,192/31,060 at 2,000 and 45,712/44,744 at 4,000. Five further pairs sampled `/proc/PID/status` at 4,000 turns: anonymous medians were 32,844/32,844 KiB and file-backed medians 13,016/12,236 KiB. Totals were lower in these runs, but that does not establish a universal RSS improvement. The old built-in zero RSS fields were not used. Raw evidence: `rss/` and `rss-breakdown/` under the same temporary directory.

Ordinary append-only history creates no revision-map entries. Genuinely replaced tool bodies carry sparse revision records, so those rows incur metadata storage; there is no second complete body. The output tests also prove transferred bytes proportional to new source, including long lines and unterminated controls, while a collapsed client's event frame stays below 400 bytes in the two-watcher fixture.

## Review and follow-up

Codex reviewed directly against `.agents/reviewers/engineering_reviewer.md`, as required by the owner's no-subagent preference. `exec018a-engineering-review.md` records the resolved findings and a final PASS verdict. Fable's independent starting-roadmap review is preserved separately; no independent review of this implementation is claimed.

EP019 still owns the daemon, bounded replay/connection queues, watch scheduling, remote handlers, deduplication and the shared client. It must bind work to the correct native transport and separate receiver/normalizer ownership before sharing real adapters: the existing BackendClient is not universally Sync. It must also finish the inventory's native-control command families and support multiple task targets rather than promote the embedded terminal's bounded selected-task cache into a global client focus. Remaining approval/interrupt/settings/lifecycle I/O is enumerated explicitly so the daemon cannot call old blocking wrappers inline.

The pre-existing queued-Claude-card-after-fork behavior and broken built-in RSS fields remain outside this plan. No Mac or iPhone build was performed. The next step is owner review of EP018a before EP019 starts.
