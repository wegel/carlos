# ExecPlan 017: build the session core without the terminal app

This ExecPlan is a living document. Keep `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` current under `.agents/PLANS.md`.

## Purpose / Big Picture


A future iPhone app needs Carlos to run sessions without terminal widgets. EP016
separated those widgets from session facts inside one executable. This plan moves
the session and backend code into a Rust library called `carlos-core`. Cargo,
Rust's build tool, will then compile and test that library without building the terminal app's widget, audio or speech libraries.
The existing
`carlos` executable will call the library and keep the same screens and backend
requests. This step adds no background server, socket protocol or phone screen.

## Progress


- [x] (2026-11-13 UTC) Read the repo rules, architecture, EP016 report, manifests and current guard. Confirmed a clean `iphone-spike` checkout at `304f494`.
- [x] (2026-11-13 UTC) Milestone 1: saved the EP016 release executable and compiler metadata under `/tmp/carlos-ep017`; added a workspace with both default members. All 1,026 existing tests pass; the empty library is only the independently buildable starting step.
- [x] (2026-11-13 UTC) Milestone 2: moved 314 Rust files and the summary prompt into `crates/carlos-core`; all 1,026 existing default tests and 990 no-dictation tests pass. Added two compile-fail documentation tests for private session facts and entry ids. Strict Clippy passes in both modes and for the standalone core.
- [x] (2026-11-13 UTC) Milestone 3: updated scripts and CI; all 26 architecture tests pass. A fresh isolated core build produced 160 compiler artifacts with no terminal, audio or speech package. A separate Rust caller creates a session and a row; the compiler rejects private-field writes, private id counters and fixture calls with test support off. Evidence is under `/tmp/carlos-ep017`.
- [x] (2026-11-13 UTC) Milestone 4: source c497daf passes all default/no-default, standalone-core, strict Clippy, release package, 174 terminal and 23 native-monitor checks. Independent speed checks clear the agreed pre-plan baseline; the card-gap append check and kernel peak-memory checks pass. Direct engineering self-review: PASS. See `.agents/reports/exec017-final-report.md`.

## Surprises & Discoveries


Before this split, the binary compiled core tests and app tests together. Several app tests
call helpers behind Rust's `cfg(test)` condition, which excludes those helpers
when a library is built as another package's dependency. The new library needs
explicit test support for those callers; copying production code into the app
would hide mistakes and is not acceptable.

The original source guard checked future-core roots under `src/` and followed eight
exact external test files. After the move it must check the real library, not an
empty old source tree. Cargo now checks the complete library dependency graph, including inactive platform and development edges, so a terminal package cannot arrive through an indirect dependency.

The package collector originally searched only inside each package for license files. The new core shares the repository license, so the manifest now declares its path and the collector follows Cargo's `license_file` field. The regression test fails against the old collector and passes after the fix.

Rust initially generated a slower synthetic append path across the new library boundary. The measured index hint removes its per-row call while retaining every check. The report includes the first slower results, rejected trials, final independent comparison and the card-gap check.

Cargo reused an older library when two source trees shared a target directory during the supplemental probe. The identical binary hashes exposed the mistake before timings were collected. Separate target directories produced distinct executables with the expected index symbols. Future source comparisons must verify the artifacts and keep their target directories separate.

## Decision Log


- Decision: keep EP017 on `iphone-spike` in `/var/home/wegel/work/perso/carlos-daemon`, without merging, pushing or installing.
  Rationale: the owner reserved `main` and the installed executable for reviewed Carlos releases; this step preserves runtime behavior.
  Date/Author: 2026-11-13, owner and primary agent.

- Decision: implement and review directly without worker or reviewer agents.
  Rationale: the owner explicitly requested no agents. The engineering prompt remains the direct review checklist; the report identifies the result as a self-review.
  Date/Author: 2026-11-13, owner and primary agent.

- Decision: keep the root `carlos` package and add `crates/carlos-core`, using resolver 2 and both default members. Create the real `carlos-proto` types in EP018.
  Rationale: Cargo can compile the core independently while existing executable paths and plain `cargo test` keep working. An empty protocol package would add no checked behavior in this step.
  Date/Author: 2026-11-13, primary agent.

- Decision: keep session fields and id allocators private; expose borrowed facts and named commands, with fixture calls behind non-default `test-support`.
  Rationale: the app needs real core calls and test helpers across the package boundary, but ordinary clients must not gain field writes or fixture mutators. The source and Cargo graph checks enforce that rule.
  Date/Author: 2026-11-13, primary agent.

- Decision: keep one `#[inline(always)]` on the unchanged entry-index insertion body.
  Rationale: the compiled builder loses its per-row library call; independent timings clear the agreed pre-plan baseline, and the batched card-gap check improves both index paths. The outer-wrapper and split-helper trials were rejected.
  Date/Author: 2026-11-13, primary agent.

## Outcomes & Retrospective


Carlos now builds a terminal-free session library and a terminal app in one Cargo workspace. Milestones 1 through 4 are complete on source `c497dafb671a4f38661c8f75e655aecb82bd6006`. Cargo keeps the existing 1,026 default tests and 990 no-dictation tests, and the compiler checks two new privacy examples. The standalone library builds without terminal, audio, speech or fixture dependencies. The full release runner passes 174 terminal workflows, installed Codex contracts and 23 native monitor workflows.

Rust needed one measured inline hint to avoid the slower row-construction path after the crate move. The source keeps every id check and exception-map rule. The final independent comparison found no repeatable latency loss against the original pre-plan baseline; it records a +0.04 ms paired construction difference from EP016 rather than claiming zero overhead. Kernel memory counts remain roughly linear with rows. The final report records every number and the rejected trials.

The primary agent implemented and reviewed directly without agents. The code stays on `iphone-spike`; the main checkout and installed binary were not changed, and nothing was merged or pushed. EP018 has not started and awaits the owner's review of this completed step.

## Reviews


Direct engineering self-review, 2026-11-13, source c497daf against 304f494, using `.agents/reviewers/engineering_reviewer.md`: PASS. The owner requested no agents, so no independent reviewer session was started. I reviewed field privacy, public calls and docs, feature isolation, all moved tests and include paths, license packaging, exact backend behavior, index code, timing and memory. The resolved findings were the missing shared license, exposed rewind internals, missing API docs and the measured row-construction cost. I found no remaining BLOCKER, MAJOR or MINOR finding in this change. The report records test commands, counts, intervals, raw evidence and the unchanged limits.

## Context and Orientation


At the start of this plan, the root `Cargo.toml` defined one binary package. `src/main.rs` declared
the terminal modules (`app`, `event`, `theme`, `clipboard`, optional `dictation`)
and the future core modules. The future core consists of `session`, `backend`,
`claude_backend`, `grok_backend`, `summaries`, `native_form` and its error helper,
the `codex_*` modules and the `protocol*` modules. Their internal imports use
`crate::`, which names the package being compiled. Preserve those imports inside
the library. App imports must instead name `carlos_core::`.

`src/session/state.rs` owns private session facts. Its methods and sibling
command modules give the app borrowed reads and named changes. `src/session/entry_id.rs`
keeps each id's namespace and counter private. The app must not gain direct field
writes or allocate ids itself. `src/session/transcript.rs` owns the contiguous
rows and unfinished-row tracker. Preserve those algorithms and their inline hints.

Core tests currently live beside their modules and in eight files under
`src/tests/`. Move those eight files under the new library's `src/tests/` and
move only fixtures they own. Shared fixture bytes may remain in the repository
with an explicit relative include. Any build-time schema include must still find
the checked-in `docs/app-server-schema` bundle. Do not change the schema.

`scripts/test.sh` runs architecture checks, Rust tests, terminal workflows and
optional backend contracts. `scripts/check-session-boundary.py` and
`tests/architecture/test_session_boundary.py` protect EP016's ownership rule.
CI lives in `.github/workflows/check.yml` and `.github/workflows/release.yml`.
Packaging scripts expect `target/release/carlos`; keep that path working.

## Plan of Work


### Milestone 1: preserve the baseline and establish the workspace


Save `target/release/carlos`, its hash, compiler version and source revision under
`/tmp/carlos-ep017`. Inventory actual core imports and app calls. Add the workspace
and a minimal documented core library with its required dependencies. Check both
packages compile and commit this independently buildable step. No placeholder
protocol or server belongs in this workspace yet.

### Milestone 2: compile the app against the real library


Move the terminal-free modules into `crates/carlos-core/src/`, with tests and
owned fixtures. Declare their root modules in `lib.rs`; make app-facing modules,
types and methods public as needed. Retain narrower visibility for storage,
allocators and backend internals. Change terminal code and app tests to import
from `carlos_core`; remove the old binary module declarations. Do not keep a
second module tree or re-export bridge in `main.rs`.

Compile test targets as well as ordinary targets. Group helpers used across the
package boundary behind `test-support`; keep unit-test modules behind `cfg(test)`.
The app's development dependency enables support, while its normal dependency
does not. Move helper prerequisites together, including deterministic pipe hooks.
Run default and no-dictation tests and strict Clippy. Preserve the combined test
count, except for clearly recorded new tests. Commit only a buildable tree.

### Milestone 3: enforce the actual package boundary


Update the guard to inspect every library source and declared test, including
conditional modules and explicit paths. Preserve its lexical negative tests and
add checks against the new layout. Check Cargo's resolved core graph, including
development/build dependencies and target branches, for terminal and audio
packages; reject unreviewed dependencies rather than allow unknown additions.
Keep fixture-only calls out of the normal library API. Verify the core alone in
an isolated Cargo target directory without the app or its default features.

Update `scripts/test.sh`, CI and style checks wherever they otherwise skip a
workspace member or search only the old source tree. Add focused negative checks
that would detect a terminal dependency or missing core tests. Run the normal
runner and confirm release packaging still uses the unchanged executable path.

### Milestone 4: verify behavior and speed, then record the result


Run the full release runner with Codex contracts, strict Clippy and both feature
sets. Compare the saved EP016 executable and current release with alternating,
same-CPU `perf-session` runs. Use the synthetic case (1,000 turns, seed 1,
24 tool lines, 160 by 48 terminal) and the checked-in stale replay fixture.
Collect at least 15 pairs, retaining counts, raw timings and individual samples.
If a median grows by more than 0.10 ms, collect independent follow-up pairs and
investigate a repeatable change. Use kernel peak memory counts at increasing
synthetic sizes; the built-in RSS helper is known to return zero and is not proof.

Review the complete diff against the engineering checklist, focusing on feature
isolation, private facts, fixtures, paths, packaging and lost tests. Fix findings
and rerun affected checks. Record all results and limits in a checked-in report.
Complete this plan's living sections, move the file unchanged to `.agents/done/`,
and update `PROGRAM_PLAN.md`. Stop after EP017 for the owner's review; do not
start EP018 or reinterpret old EP009-015 files as this spike's next task.

## Concrete Steps


Run commands from `/var/home/wegel/work/perso/carlos-daemon`, prefixing shell
commands with `rtk` under `~/.codex/RTK.md`. Use `semeja search` for code by intent
and exhaustive literal searches only for import/path inventories. Key commands:

    rtk proxy cargo test
    rtk proxy cargo test --workspace --no-default-features
    rtk proxy cargo clippy --workspace --all-targets -- -D warnings
    rtk proxy cargo clippy --workspace --all-targets --no-default-features -- -D warnings
    rtk proxy cargo tree -p carlos-core --edges normal,build,dev
    rtk proxy cargo build -p carlos-core --target-dir /tmp/carlos-ep017/core-target
    rtk proxy bash scripts/test.sh --release --codex-contract
    rtk proxy cargo fmt --all --check
    rtk proxy git diff --check

The final comparison ran each baseline and current executable with these arguments:

    rtk proxy target/release/carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48
    rtk proxy target/release/carlos perf-session src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48

The paired runner pinned each process to CPU 6 and rotated executable order for 31 samples per case. The supplemental append check used ten batches of 50,000 rows per sample. Retain the raw evidence under `/tmp/carlos-ep017`. Tests must show the library and app
test processes separately, with at least 1,026 combined passing tests by default.

## Validation and Acceptance


A caller can run `cargo build -p carlos-core` without building Carlos's terminal
or dictation dependencies. Its resolved graph contains no terminal, audio or
speech packages. A fresh build directory proves this independently of cached
app artifacts. Plain `cargo test` runs both package suites. The root executable
keeps its name, CLI, default dictation feature, release build paths and package
helper behavior. Every existing terminal and native-monitor workflow must pass.
The tests must preserve entry identity, callback order and backend request bytes.
The app can borrow facts and submit commands but cannot mutate session fields.

The final report records commands, source revision, combined counts, release
binary hash, dependency graph and paired speed/memory evidence. A repeatable
slowdown, dropped workflow or unresolved engineering blocker prevents completion.

## Idempotence and Recovery


Use explicit paths in `git add`; never use `git -A`. Keep small buildable commits
on `iphone-spike`. Repeated Cargo and test commands are safe. Preserve the saved
baseline and raw evidence under `/tmp/carlos-ep017`. If a move fails to compile,
fix imports and feature gates in this checkout; do not reset the owner's checkout
or replace the baseline. Do not edit completed EP016 or historical reports.

## Artifacts and Notes


The starting commit is `304f494`; EP016's accepted source is `84f868d`. Its release
binary hash is `d0a59b4fd4342d7b20ad22db7bc7094b590e60a0e0bbc44ec530a198d2cf9cc2`.
Earlier evidence remains in `/tmp/carlos-ep016-final`, and the complete handoff is
`.agents/reports/exec016-final-report.md`. Keep new evidence separate.

## Interfaces and Dependencies


The new `crates/carlos-core/Cargo.toml` defines package `carlos-core`, library
import name `carlos_core`, edition 2021 and no default features. Its optional
`test-support` feature has no terminal dependency. Keep current third-party
versions and the lockfile; move actual backend dependencies from the binary to
this package. Do not add a second async runtime or change backend JSON.

`carlos_core::session::state::SessionState` keeps its private fields and existing
read/command methods. `carlos_core::backend::BackendClient` remains the backend
trait. `carlos_core::session::entry_id::EntryId` stays opaque, with private
namespace and counter and no public allocator. The `carlos` package retains
terminal rendering, clipboard, input, local drafts and optional dictation.

Plan written 2026-11-13: scoped EP017 to a compiler-enforced library boundary,
with explicit checks for the tests and feature gates that EP016's report flagged.

Milestone 1 update, 2026-11-13: preserved the baseline and verified plain Cargo commands discover both packages before moving source.

Milestones 2 and 3 update, 2026-11-13: preserved every existing test, kept root executable paths, and replaced the old future-core check with checks of the actual library and Cargo graph. The isolated caller checks prove useful terminal-free behavior and private fields across the real package boundary.

Release packaging discovery, 2026-11-13: the first release runner reached package staging and rejected carlos-core because its license lives at the repository root. The core manifest now declares `license-file = "../../LICENSE"`; the collector copies Cargo-declared files as well as files found in each package directory. A temporary-workspace regression test reproduces that case. The final release run remains open.

Direct-review follow-up, 2026-11-13: moved the pure native rewind guard tests into the library too, instead of exposing internal guard modules through test support. The compiler now keeps both native rewind modules private. This ninth formerly external core test file preserves the combined test count.

Public API documentation, 2026-11-13: the direct review found newly public declarations without explanations. Added one-sentence docs to those types, methods and backend trait calls, and documented standalone core commands in README.md. Runtime tokens did not change. The full release runner already passes all workflows; the final release executable will be rebuilt with these comments before measurement.

Speed gate still open, 2026-11-13: the first 31 quiet EP016/current pairs show synthetic apply medians 14.11/14.26 ms (paired +0.18 ms, bootstrap 95% interval +0.10 to +0.33). Total replay is 47.10/46.72 and layout 14.87/14.59 ms. Row counts match. The apply result crosses the 0.10 ms follow-up threshold, so completion is withheld pending independent pairs and, if repeatable, a compiler-boundary fix. The RSS medians are recorded under `/tmp/carlos-ep017/rss`; keep those raw results even if later code changes.

Repeated speed finding, 2026-11-13: another 31 alternating pairs reproduce the apply cost (14.15/14.42 ms, paired +0.22, 95% interval +0.15 to +0.32). The release function that contains the synthetic builder grew from 0x6525 to 0x77c6 bytes after the package move. Testing one stronger inline hint on the existing session append wrapper; retain the unoptimized executable and both measurement sets. No acceptance claim yet.

Inline trial correction, 2026-11-13: forcing the outer append wrapper produced a byte-identical executable, so that hint was reverted. The lower apply median in that run cannot be credited to the hint. The next trial makes the index fast path available across the crate boundary and leaves exception-map insertion in its own helper; it preserves every id check and reserve rule. This addresses the six remaining index calls found in the builder assembly rather than changing the synthetic fixture.

Index-helper trial, 2026-11-13: the separated fast path removed the index call but did not remove the measured apply cost (31 pairs: 14.10/14.28 ms). Reverted that extraction. Next test: make the complete existing index insertion body inline at its constructor caller so the compiler can remove redundant checks itself, without removing checks from the source.

Index inline result, 2026-11-13: one `#[inline(always)]` on the unchanged index insertion body removes the per-row external call. All 1,026 default tests and two privacy documentation tests pass. In 31 quiet pairs, EP016/current apply is 14.06/14.12 ms; paired change +0.06 ms, bootstrap 95% interval -0.01 to +0.15. No measured latency interval has a positive lower bound. Total replay is 46.93/46.47 ms. Keep this measured hint; final independent timings and complete checks remain open.

Supplemental card-gap check, 2026-11-13: Cargo reused an old library when two checkouts shared a target directory, producing identical probe executables. Discarded that comparison before collecting timings. The old and current libraries now build in separate target directories with Rust 1.100.0 pinned; verify their symbols and hashes before measuring dense appends and appends after a real card id creates a counter gap.

Final closeout, 2026-11-13: completed all four milestones and recorded the direct review and final evidence in `.agents/reports/exec017-final-report.md`. The complete plan is moved unchanged to `.agents/done/`; the owner reviews EP017 before EP018 starts.

Supplemental result, 2026-11-13: 31 alternating pairs of ten 50,000-row batches show before/current medians 7.31/7.17 ms for ordered rows and 19.91/18.98 ms after a card gap, with both paired intervals favoring the hint. All row/id assertions pass. The final source, report, review and independent timing/memory checks are complete.

Archive note, 2026-11-13: the final report and completed status supersede the dated in-progress gate notes while retaining their history. The file is frozen before moving to `.agents/done/`.
