# ExecPlan 016: final checks

Carlos now keeps session facts and backend handlers in SessionState, while ViewState keeps terminal widgets, unsent text and local cursor/scroll choices. The transcript assigns opaque EntryId values; native-key maps and delayed work keep those values. The Claude backend translates native frames into the existing JSON item, turn, usage and approval messages before the session handles them. Carlos still uses one package and one session per process.

The tested source is 84f868d83b5dfaa30770006420bb2374758626a5. The pre-plan source is a59221cc7dc7e619bc6dce961224a50f6a54be9b. Both speed endpoints use default features, thin LTO, one codegen unit and Rust 1.100.0 (0ef1bc386, LLVM 23.1.5). The installed compiler matches the [official 1.100.0 manifest](https://static.rust-lang.org/dist/channel-rust-1.100.0.toml), dated 2026-11-12. CI and rust-toolchain.toml select that version.

Codex continued the recovered Claude handover directly, following the owner's request to use no worker or reviewer agents. The code and reports remain on daemon-split. The installed Carlos binary and the main checkout were not changed; the branch was neither merged nor pushed. ExecPlan 017 has not started.

The following commands passed on the final source:

- cargo test: 1,026 passed, 3 ignored. The test process took 3.78 seconds in the source-commit check.
- cargo test --no-default-features: 990 passed, 2 ignored. The final quiet run took 3.77 seconds.
- cargo clippy --all-targets -- -D warnings, with default features and with --no-default-features.
- cargo build --release, with default features and with --no-default-features. Cargo then restored the default executable.
- bash scripts/test.sh --release --codex-contract: 16 architecture tests, 1,026 release Rust tests, 174 terminal workflows, installed Codex 0.167.1 contracts and 23 native monitor workflows passed. The runner exercised target/release/carlos and the packaged controller with fixed peers and isolated Codex homes, without inference or user sessions.
- cargo fmt --check and git diff --check.
- The complete source-size check against a59221c found no changed Rust file above 400 lines and no function above 60 lines.

The guard reads every Rust file in the future-core roots and follows conditional modules, inline modules and declared path children. It admits eight exact external test files. It rejects terminal crates, client imports, unreviewed dependencies, include!, unchecked module paths and SessionState impls outside the session tree. The tests cover aliases, re-exports, nested comments, raw strings, lifetimes, target-specific Cargo aliases and the actual repository. The app's migration bridges are removed. The app's forwarder tests still use the real reader threads and production forwarder through a terminal-free harness.

The harness replayed all 24,300 original streams and 600 additional callback/form streams. The final source matched parent 3d58606 at all 449,884 checkpoints across all 24,900 streams, including backend calls, identity relationships, drafts, rendered output and redraw logs. Neither endpoint panicked. The harness normalizes the random UUID namespaces while retaining id reuse and retirement relationships. Its adapter translates the corpus's former low-level rewind-index command into a resolved anchor inside the archived test tree. Separate headless and terminal tests exercise the public EntryId command, including unbound prompts and stale ids.

The final program differs from the immutable pre-plan program in 1,016 streams under rules already accepted earlier in the plan:

- In 878 streams, the newer history builder omits redraw calls from a discarded scratch view; stored facts and rendered output match.
- In 46 streams, the session refuses history from another thread before changing the current rows. All 44 cases absent from the old primary-cause list first differ at such a foreign history after a fork. Eleven former primary B cases retain only scratch redraws; one retains this foreign-history refusal. The mixed-backend cursor defect remains fixed.
- In 24 streams, the session reuses a row for a repeated native Claude assistant source.
- In 67 synthetic Claude streams, the backend fails an active turn that the mock never registered, rather than leaving that turn active after closure.
- In one stream, the store gives a reappearing, previously retired diff part a fresh id.

The unfinished-row tracker keeps only unfinished offsets inside the transcript store. The renderer visits those offsets from the latest one, rather than scanning completed rows. Completion and truncation remove them; native maps and callers keep EntryId. Two direct tests cover empty placeholders, completion, truncation and late events targeting a replacement row. The label helper writes into one string buffer and preserves the exact label bytes.

Prompt history owns a separate id allocator for pending records. Pending records therefore neither consume the transcript counter nor force later row lookups into its exception hash map. The new test queues 100 pending records between two rows. It fails on 31d400f with row-counter offset 101 instead of 1 and passes on the final source. The view still sends the stable prompt id and edited text; the session resolves native fork positions and the original text. Transcript rows and approval cards retain their earlier allocator rules.

The final quiet speed run alternated the two release executables on CPU 6 for 31 samples per endpoint and case. The synthetic case used seed 1, 1,000 turns, 24 tool lines and a 160 by 48 viewport. Both programs reported 6,001 rows, 39,347 rendered lines and 6,000 relevant items. The synthetic builder went through the real entry allocator and index. Values below are pre-plan/current milliseconds:

- Apply: 14.12/14.06; total replay: 46.92/47.22.
- Full layout: 14.92/14.94; full draw: 3.47/3.44.
- Scroll p50/p95: 0.71/0.82 and 0.71/0.82.
- Typing p50/p95: 0.60/0.61 and 0.60/0.61.
- Working p50/p95: 0.60/0.61 and 0.60/0.61.
- Append p50/p95: 0.61/0.66 and 0.61/0.67.
- Median per-run maxima for scroll, typing, working and append: 0.84, 0.65, 0.65, 0.66 versus 0.87, 0.66, 0.66, 0.67.

The paired median apply change was -0.05 ms, with a bootstrap 95% interval of -0.13 to +0.02 ms. The paired median replay change was +0.24 ms, with an interval of -0.05 to +0.45 ms. Every measured latency interval includes zero. Because the raw replay median crossed the plan's 0.10 ms follow-up trigger, Codex collected three further alternating pairs: replay 48.07/46.63, apply 14.17/14.12, layout 15.46/14.73, draw 3.59/3.63. The earlier slower replay median did not repeat. The small fixture retained five rows, nine rendered lines and four relevant items, with replay 1.00/0.99 ms. Codex detected no repeatable slowdown; the timing data still has ordinary host noise and is not a claim of exactly zero overhead.

The Linux kernel reported peak resident memory through os.wait4, with five alternating pairs at each size. Median pre-plan/current KiB were 23292/23860 at 1,000 turns, 29976/30888 at 2,000 and 43764/44668 at 4,000. Peaks across those five samples were 23504/24564, 30356/31232 and 44024/44964 KiB. The final program uses more memory for ids and state, and both programs grow roughly linearly with rows. The existing perf-session RSS fields still print zero and were not used as evidence.

Codex reviewed the complete outcome and the changed handlers with the engineering reviewer checklist. Codex checked callback precedence, native replay acknowledgements, nested history actions, failed sends/forks, same-thread identity, retired ids, quota readiness, queue eligibility, source dependencies and cache resets. Codex corrected the redundant fork redraw, moved quota readiness into session commands and separated pending prompt ids from row counters. Codex found no remaining blocker in the changed code. Verdict: PASS. This was a direct self-review, following the owner's instruction to work without agents.

The earlier queued-Claude-card-after-fork defect and broken RSS helper remain recorded limits from before this change. The next author must adjust crate exports and expose test support when the app tests call helpers in a separate library. Carlos still uses the current JSON receiver and immediate local change reports; this plan adds no daemon or new wire protocol.

Raw evidence remains under /tmp/carlos-ep016-final: namespace-release-contract.log, namespace-no-default-quiet.log, namespace-full-run.log, full/final-current-comparison.json, full/classified-comparison.json, classify/report.json, namespace-quiet, namespace-rss and accepted-source.json. The original corpus byte hash is 0ff34c257a5acb7d0e5f787f4057fa03a8c757577d5091f5d58bcc4890dcfeb8. The final executable SHA-256 is d0a59b4fd4342d7b20ad22db7bc7094b590e60a0e0bbc44ec530a198d2cf9cc2.
