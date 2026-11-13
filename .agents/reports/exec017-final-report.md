# ExecPlan 017: terminal-free session library

Carlos now builds its session and backend code as `carlos-core`, a Rust library under `crates/carlos-core`. The root `carlos` package keeps terminal widgets, clipboard handling and optional dictation. Cargo builds both packages together, and plain `cargo test` runs both suites. The app calls the library directly; the old binary module declarations and import bridges are gone. Carlos still runs one session per process and uses the same backend JSON messages. EP018 has not started.

The final source is `c497dafb671a4f38661c8f75e655aecb82bd6006`. The starting checkout was `304f494` on `iphone-spike`. Both release baselines and the final executable use Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5), thin link-time optimization, which lets Rust optimize calls between compiled packages, and one code-generation unit per package. The Linux executables contain matching compiler version records. The final release SHA-256 is `a8cd1d59293c0108c7942bde66156ed21155a3a1fa67cb91fc014a70bc803078`.

The following checks passed:

- Plain `cargo test`: 811 app tests and 215 core tests passed, retaining all 1,026 existing passing tests and three existing ignored tests. Two new documentation tests ask the compiler to reject access to private session fields and entry-id counters.
- `cargo test --workspace --no-default-features`: 775 app tests and 215 core tests passed, plus the two privacy documentation tests; two existing tests remain ignored.
- Rust's Clippy checker with `--workspace --all-targets -- -D warnings`, both with and without default features. The standalone core also passed strict Clippy and its 215 tests without enabling test support.
- Default and no-dictation release builds. The final default app build reports features `default, dictation`; its core reports only the empty `default` feature. Test helpers are absent from ordinary builds.
- `bash scripts/test.sh --release --codex-contract`: 26 architecture/package checks, 1,026 optimized Rust tests, 59 tests for the Claude helper and its bundled library per helper pass, package smoke checks, 174 terminal workflows and 23 native monitor workflows passed. The installed Codex contracts passed with temporary generated sessions and local peers, without paid inference or user sessions.
- `cargo fmt --all --check`, whitespace checks and checks of Rust file and function lengths. No changed Rust file exceeds 398 lines and no function exceeds 60 lines.

Cargo produced 160 compiled files in a fresh directory for the core, without compiling a terminal, audio or speech package. The Cargo package checker also follows packages needed for tests, builds and other operating systems, rejects renamed terminal packages and requires empty core default features. The source guard checks every library Rust file, including undeclared files and conditional/path modules, and still rejects client types and outside session impls.

A separate Rust caller created a session and one row using the ordinary core library. The compiler rejected direct writes to `SessionState.thread_id`, reads of the private entry counter and calls to fixture mutators with test support off. The app receives borrowed facts and named commands; session storage and id allocators remain private. App tests enable `test-support` only through a development dependency. Nine formerly external core test files now live with the library, including the pure native rewind tests, so native rewind modules remain private.

I compared all 602 original Rust files. Handler bodies keep their behavior. The runtime and fixture string literals match the starting source, apart from two checked relative include paths for the schema and frozen label fixture. `Transcript::default` and `RalphRuntimeState::default` call existing constructors. The one runtime compiler hint is described below; the plan records the rejected trials.

The release package initially failed because the new library shares the root license. `carlos-core` now declares `license-file = "../../LICENSE"`, and the notice collector copies Cargo-declared license files. The new temporary-workspace test reproduces the missing-license failure on the old collector and passes with the fix. The full package runner passes with the changed collector.

The first workspace executable took about 0.18 to 0.22 ms more than EP016 to build 1,000 synthetic turns. A stronger hint on the outer append wrapper produced an identical executable and was removed. Splitting the index's map path also failed to remove the cost and was reverted. One `#[inline(always)]` on the existing `EntryPositions::insert` body removed the per-row library call without changing its checks, map reserve rules or id allocator. All final checks include that hint.

The independent speed run used 31 samples per executable and case, rotating the order of the pre-plan, EP016 and current executables on CPU 6. The synthetic case used 1,000 turns, seed 1, 24 tool lines and a 160 by 48 viewport. Every executable retained 6,001 rows, 39,347 rendered lines and 6,000 relevant items. The small saved-session fixture retained five rows, nine rendered lines and four relevant items. Values below are pre-plan/EP016/current median milliseconds:

- Synthetic row construction: 14.16/14.05/14.10.
- Total synthetic replay: 46.90/46.90/47.06.
- Full layout: 14.89/14.82/14.74; full draw: 3.48/3.44/3.48.
- Scroll p50/p95: 0.71/0.82 for all three executables.
- Typing and working p50/p95: 0.60/0.61 for all three.
- Append p50/p95: 0.61/0.66 for all three.
- Small-fixture replay: 1.02/1.06/0.99.

I resampled the paired timing differences to estimate 95% intervals. Against the agreed pre-plan baseline, the paired difference in time to build rows was -0.06 ms, with an interval of -0.16 to +0.04 ms. I found no repeatable increase in any measured time against that baseline. Against EP016, the paired construction change was +0.04 ms, with an interval of +0.01 to +0.07 ms; this small difference remains below the plan's 0.10 ms follow-up threshold. Because the raw total-replay median crossed that threshold, I collected three further paired samples: pre-plan/EP016/current replay 47.08/47.39/47.12 ms and construction 14.13/14.31/14.11 ms. The larger total-replay gap did not repeat. Raw samples retain the slower runs too.

I also compared the workspace library before and after the index hint. Each sample appended ten batches of 50,000 real rows, either in counter order or after a real card id created a gap. Each batch checked the first and final row positions and confirmed that the card id never resolved to a row. Both libraries used Rust 1.100.0, thin LTO and separate target directories. In 31 alternating pairs, before/current medians were 7.31/7.17 ms for ordered rows and 19.91/18.98 ms after a gap. The paired 95% intervals were -0.56 to -0.01 ms and -1.57 to -0.52 ms. An initial shared-target build reused the old library and produced identical probe hashes; that comparison was discarded before collecting timings.

The Linux kernel supplied peak resident memory through `os.wait4`, with five alternating pairs per size. EP016/current median KiB were 23892/24016 at 1,000 turns, 30744/30796 at 2,000 and 44652/44432 at 4,000. The largest samples were 24272/24132, 30924/30828 and 44968/44624 KiB. Both executables use memory roughly in proportion to the number of rows. The built-in RSS columns still print zero and were not used as evidence.

I reviewed the complete change directly with `.agents/reviewers/engineering_reviewer.md`, following the owner's instruction to use no agents. I checked private fields, public calls and their docs, ordinary versus test-only features, every moved test and include path, package licenses, backend request behavior, compiler hints, speed and memory. The resolved findings were the shared license, unnecessary exposure of rewind internals, missing API explanations and the measured constructor cost. I found no remaining blocking finding. Verdict: PASS. This was a self-review, not an independent reviewer session.

I kept the behavior and known limits recorded in EP016. EP018 must introduce numbered core events and define the first client protocol; this plan creates no empty protocol package or daemon. The branch remains local, the main checkout and installed binary were not changed, and nothing was merged or pushed. The owner reviews each ExecPlan before the next one starts.

Raw evidence remains under `/tmp/carlos-ep017`: `accepted-source.json`, `accepted-app-build.jsonl`, `accepted-core-artifacts.json`, `accepted-core-tests.log`, `accepted-consumer.log`, `final-release-contract.log`, `accepted-no-default-*`, `literal-audit.json`, `source-audit.json`, `function-sizes.json`, `holdout/`, `followup/`, `rss-final/` and `index-probe/batched/`. The earlier `perf*` sets and trial executables remain available. The release runner records terminal screens, keys and both sides of backend traffic under the artifact directory printed in its log.
