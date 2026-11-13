# ExecPlan 017: build the session core without the terminal app

This ExecPlan is a living document. Keep `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` current under `.agents/PLANS.md`.

## Purpose / Big Picture


A future iPhone app needs Carlos to run sessions without terminal widgets. EP016
separated those widgets from session facts inside one executable. This plan moves
the session and backend code into a Rust library called `carlos-core`. Cargo,
Rust's build tool, will then compile and test that library without downloading or
building the terminal app's widget, audio or speech libraries. The existing
`carlos` executable will call the library and keep the same screens and backend
requests. This step adds no background server, socket protocol or phone screen.

## Progress


- [x] (2026-11-13 UTC) Read the repo rules, architecture, EP016 report, manifests and current guard. Confirmed a clean `iphone-spike` checkout at `304f494`.
- [x] (2026-11-13 UTC) Milestone 1: saved the EP016 release executable and compiler metadata under `/tmp/carlos-ep017`; added a workspace with both default members. All 1,026 existing tests pass; the empty library is only the independently buildable starting step.
- [x] (2026-11-13 UTC) Milestone 2: moved 313 Rust files and the summary prompt into `crates/carlos-core`; all 1,026 existing default tests and 990 no-dictation tests pass. Added two compile-fail documentation tests for private session facts and entry ids. Strict Clippy passes in both modes and for the standalone core.
- [x] (2026-11-13 UTC) Milestone 3: updated scripts and CI; all 24 architecture tests pass. A fresh isolated core build produced 160 compiler artifacts with no terminal, audio or speech package. A separate Rust caller creates a session and a row; the compiler rejects private-field writes, private id counters and fixture calls with test support off. Evidence is under `/tmp/carlos-ep017`.
- [ ] Milestone 4: run complete release workflows, compare speed and memory, review directly, and close the plan.

## Surprises & Discoveries


The current binary compiles core tests and app tests together. Several app tests
call helpers behind Rust's `cfg(test)` condition, which excludes those helpers
when a library is built as another package's dependency. The new library needs
explicit test support for those callers; copying production code into the app
would hide mistakes and is not acceptable.

The original source guard checked future-core roots under `src/` and follows eight
exact external test files. After the move it must check the real library, not an
empty old source tree. Cargo must also check the complete library dependency graph
so a terminal package cannot arrive through an indirect dependency.

## Decision Log


The owner chose branch `iphone-spike` for EP016 through EP022. Work stays in
`/var/home/wegel/work/perso/carlos-daemon`; do not touch the main checkout, push,
merge, or install this experimental binary. This plan preserves runtime behavior,
so the prompt's rule about installing changed runtime behavior does not apply.
The architecture also reserves installation for the owner's review.

The owner asked the primary agent to work directly without agents. That preference
continues here. Use `.agents/reviewers/engineering_reviewer.md` as a direct review
checklist and record a self-review honestly instead of starting a reviewer agent.

Keep the `carlos` package in the repository root and add `crates/carlos-core` as a
workspace member. A workspace lets Cargo build packages together while keeping
their dependencies separate. Use dependency resolver 2 and include both packages
in default workspace commands so plain `cargo test` cannot skip core tests.
Keep the existing release profile and executable paths. Create `carlos-proto`
with the actual numbered event types in EP018, rather than an empty package here.

Expose only library modules and items used by the app or required public types.
Keep `SessionState` fields restricted to its session module and keep entry-id
allocators private. Use a non-default `test-support` feature only as the app's
development dependency. Ordinary builds must not expose fixture mutators or
compile test peers. All runtime handlers remain one copy in the library.

## Outcomes & Retrospective


Milestones 1 through 3 are implemented and checked. The terminal app now calls the library; both feature sets keep the existing unit tests. The compiler rejects client writes to private facts and excludes fixture helpers from the ordinary library. Final release workflows, speed/memory checks and direct review remain open.

Historical starting point: EP016 ended with 1,026 default Rust
tests and 990 tests without dictation, 174 terminal workflows and 23 native monitor
workflows. Its release executable is available locally and will be copied before
any build replaces it. The next action is to inventory imports and save that
baseline, then move the terminal-free modules and their owned test fixtures.

## Context and Orientation


The root `Cargo.toml` currently defines one binary package. `src/main.rs` declares
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

Record exact perf commands after checking `carlos perf-session --help`; retain
the old release executable before compiling. Tests must show the library and app
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
