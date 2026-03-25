# Make large sessions stay smooth in the `carlos` TUI

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

This document must be maintained in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, `carlos` should remain responsive when a session grows very large. A user
should be able to resume a big session, type into the input box, scroll the transcript, and
watch live updates land without the interface feeling sticky or pegging a CPU core for avoidable
redraw work. The proof is behavioral and measurable: `cargo test` stays green, the TUI still
renders the same transcript content correctly, and `carlos perf-session` shows substantially
lower work on the hot paths that currently rebuild too much state.

## Progress

- [x] (2026-03-24 00:00Z) Added an offline perf harness with `carlos perf-session` so large
  sessions can be benchmarked without manually driving the TUI.
- [x] (2026-03-24 00:00Z) Added deterministic synthetic perf sessions with `--synthetic`,
  `--turns`, `--seed`, and `--tool-lines` so perf work can be reproduced on any machine.
- [x] (2026-03-24 00:00Z) Removed render-time collapsing of repeated `→ Read ...` summaries and
  moved that coalescing into message/state updates.
- [x] (2026-03-24 01:00Z) Replaced `ensure_rendered_lines()` full rebuilds for append and
  last-message update paths with dirty-from incremental invalidation backed by per-message
  rendered blocks and block offsets.
- [x] (2026-03-24 01:20Z) Removed the remaining normal-redraw whole-history clone/filter path by
  making rendering and selection read from cached message blocks plus block offsets instead of a
  flattened transcript snapshot.
- [x] (2026-03-24 01:40Z) Reduced active-turn redraw pressure without degrading the animation:
  the working separator still animates at the normal cadence, but animation frames now rely on
  on-demand cached layout and benchmark at the same cost as ordinary viewport draws.
- [x] (2026-03-24 01:00Z) Captured before/after perf evidence from both the synthetic source and
  the large real captured session `019c6286-d480-7293-8fd8-bd6459fab3ad`.
- [x] (2026-03-24 02:20Z) Split full-layout into an eager line-count prepass plus lazy block
  materialization so resume no longer builds every rendered line up front.
- [ ] Shrink the initial full-layout cost further; it improved materially, but it is still about
  `2.8 s` on the captured 4M-line session.

## Surprises & Discoveries

- Observation: the first synthetic `perf-session` implementation was itself too expensive because
  it replayed every historical item with a full draw at each step.
  Evidence: the initial harness pegged a core and had to be redesigned to load once and then
  benchmark sampled scroll, typing, and append operations.

- Observation: even after removing render-time read-summary collapse, the append benchmark is
  still dominated by full transcript layout work.
  Evidence: `carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160
  --height 48` reported `append_total: p50 91.70 ... max 102.79 ms` while `full_layout` was
  `97.08 ms`, which means the next bottleneck is the whole-transcript rebuild in
  `AppState::ensure_rendered_lines`.

- Observation: the append benchmark in `perf-session` initially kept forcing a full rebuild even
  after the incremental cache existed, because the harness still called `mark_transcript_dirty()`
  instead of the new dirty-from path.
  Evidence: the first post-cache synthetic run still showed `append_total: p50 97.30 ms`; after
  fixing the harness to call `mark_transcript_dirty_from(idx)`, the same run dropped to
  `append_total: p50 0.76 ms`.

- Observation: the incremental cache makes the hot append and last-message update path fast, but
  it does not yet solve the initial full layout cost for giant histories.
  Evidence: the real captured session
  `/home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl`
  still reports `full_layout: 4348.32 ms` for `139591` messages and `4078383` rendered lines,
  even though `append_total`, `typing_draw`, and `scroll_draw` are all around `0.6 ms`.

- Observation: once layout work is gated behind invalidation, the animated working separator is
  no longer meaningfully more expensive than ordinary redraws, even on the giant captured
  session.
  Evidence: after the main-loop/layout change, the real captured session reports
  `working_draw: p50 0.58 p95 0.59 avg 0.58 max 0.63 ms`, essentially identical to
  `typing_draw` and `scroll_draw`.

- Observation: eagerly computing exact line counts for the whole transcript is much cheaper than
  eagerly materializing every `RenderedLine`, even before deeper count-only parser work.
  Evidence: after changing `ensure_rendered_lines()` to store block offsets plus line counts and
  materialize only the visible blocks on demand, the real captured session `full_layout` dropped
  from `4326.11 ms` to `2828.81 ms` while `working_draw`, `typing_draw`, and `append_total`
  stayed around `0.60 ms`.

## Decision Log

- Decision: build a deterministic synthetic perf source before deeper render refactors.
  Rationale: captured `~/.codex/sessions/...jsonl` files are useful but not portable or
  reproducible. A synthetic source lets perf regressions be reproduced on any checkout and in CI.
  Date/Author: 2026-03-24 / Codex

- Decision: remove whole-history transformations from the redraw path one by one instead of
  attempting an immediate full renderer rewrite.
  Rationale: this keeps correctness easier to verify and lets each optimization prove whether it
  actually changes the hot-path measurements.
  Date/Author: 2026-03-24 / Codex

- Decision: accept a small increase in full synthetic layout cost in exchange for collapsing the
  hot append and last-message update path from tens of milliseconds to sub-millisecond work.
  Rationale: live interaction smoothness matters more than one-time resume cost for this
  milestone, and the measurements show the new cache materially improves the interactive path.
  Date/Author: 2026-03-24 / Codex

- Decision: keep the working animation enabled at its normal cadence and instead make each frame
  constant-time with respect to transcript size.
  Rationale: the user requirement is that animation itself must not be a scalability problem. A
  size-based fallback would hide the bug instead of fixing it. Measuring `working_draw`
  separately keeps that invariant explicit.
  Date/Author: 2026-03-24 / Codex

- Decision: separate “known scroll bounds” from “fully materialized rendered transcript”.
  Rationale: resume performance was dominated by building and storing millions of `RenderedLine`
  values that are not visible initially. Keeping exact per-message line counts while lazily
  building visible blocks preserves correct scrolling and selection semantics without paying the
  full materialization cost up front.
  Date/Author: 2026-03-24 / Codex

## Outcomes & Retrospective

The work is still in progress, but the interactive path is now much closer to the target and the
one-time resume cost is lower than before. The append path, scroll path, typing path, and
active-turn animation path are all under a millisecond in the perf harness, including the
captured 4M-line session, and the initial full-layout cost fell by about `1.5 s`. The main
remaining risk is that `2.8 s` resume cost is still too high for the largest histories.

## Context and Orientation

`carlos` is a Rust terminal user interface. Transcript state lives in `src/app/state.rs` in
`AppState`. Rendering logic lives in `src/app/render.rs`. Incoming app-server messages are
handled in `src/app/notifications.rs`. The main event loop is in `src/app/input.rs`. Performance
metrics helpers live in `src/app/perf.rs`, and the offline benchmarking harness lives in
`src/app/perf_session.rs`.

The current architecture stores the logical transcript in `AppState.messages` and a flattened,
wrapped transcript in `AppState.rendered_lines`. The method `AppState::ensure_rendered_lines()`
rebuilds `rendered_lines` by calling `build_rendered_lines_with_hidden()` over the full
transcript when the transcript is dirty, the width changes, or a user message is hidden during
rewind. This whole-transcript rebuild is the main performance problem for large sessions.

The repository now includes `carlos perf-session`, which can replay a real captured session file
or generate a deterministic synthetic one. This tool is the required way to measure progress for
this ExecPlan.

At the start of this plan, the working tree already contains:

- the synthetic perf harness CLI and implementation
- tests proving synthetic reproducibility
- state-time coalescing of repeated read summaries

Those changes must be preserved or improved, not removed.

## Plan of Work

The first milestone is to make transcript layout incremental. In `src/app/state.rs`, replace the
single `transcript_dirty` boolean with enough information to know which message index first needs
re-layout. The common cases are append-only growth, last-message delta updates, and occasional
mid-transcript mutations such as rewind hiding or tool-call replacement. The target behavior is
that appending or extending the last message only re-lays out the affected tail, then updates the
flattened line index without touching earlier stable lines.

The next milestone is to shrink the one-time full layout cost on giant histories further. The
renderer no longer clones the message list during normal redraws, the working animation now
benchmarks as ordinary viewport work, and full layout now means “count everything, materialize
only the visible blocks.” The remaining bottleneck is the eager count prepass itself, especially
for markdown, ANSI, and diff-heavy transcripts.

At each milestone, add focused tests in `src/tests.rs` and capture perf evidence with
`carlos perf-session` against both a deterministic synthetic source and a real recorded session
when available.

## Concrete Steps

Work from the repository root `/var/home/wegel/work/perso/carlos`.

1. Run the baseline synthetic harness before a major change:

       cargo build --release
       target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48

   Record `full_layout`, `scroll_draw`, `typing_draw`, and `append_total` in this ExecPlan.

2. Read the current hot path:

       rg -n "ensure_rendered_lines|build_rendered_lines_with_hidden|rendered_lines" src/app

   Then inspect the specific functions in `src/app/state.rs`, `src/app/render.rs`, and
   `src/app/input.rs`.

3. Implement one bounded optimization step at a time. After each meaningful change:

       cargo fmt
       cargo test
       cargo build --release
       target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48

   If runtime behavior changed, also install the release binary:

       cp target/release/carlos ~/.local/bin/carlos.new
       mv ~/.local/bin/carlos.new ~/.local/bin/carlos

4. When a change is large enough to deserve review, run the spec reviewer and engineering
   reviewer in separate sessions and copy their verdicts into this ExecPlan.

## Validation and Acceptance

This ExecPlan is complete only when all of the following are true:

- `cargo test` passes.
- `cargo build --release` passes.
- `carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height
  48` shows materially lower append and layout costs than the current baseline.
- Resuming and interacting with a large real session remains visually correct: no lost lines, no
  broken selection behavior, no regressions in commentary/reasoning/tool rendering, and no
  approval flow regressions.
- The transcript does not perform obvious whole-history display transforms during normal redraws.

The human-verifiable acceptance case is: start a fresh `carlos`, resume a large session, type in
the input field, scroll up and down, and observe that the interface remains smooth while the
content remains correct.

## Idempotence and Recovery

The perf harness commands are safe to rerun. They do not mutate repository source files. The
release install step is idempotent when done through the `.new` rename path. If an optimization
attempt makes performance worse or breaks transcript behavior, revert only the bounded change and
rerun `cargo test` plus the perf harness before proceeding.

## Artifacts and Notes

Baseline synthetic evidence before the incremental cache:

    target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48
    carlos perf-session
    source: synthetic seed=1 turns=2000 tool_lines=24
    viewport: 160x48
    transcript: messages=12000 rendered_lines=80727 relevant_items=12000 replay_elapsed_ms=127.60
    full_layout:   97.08 ms
    full_draw:     0.76 ms
    scroll_draw:   p50 0.71 p95 0.74 avg 0.72 max 0.83 ms
    typing_draw:   p50 0.68 p95 0.68 avg 0.68 max 0.72 ms
    append_total:  p50 91.70 p95 102.79 avg 92.91 max 102.79 ms

Current synthetic evidence after the incremental cache:

    target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48
    carlos perf-session
    source: synthetic seed=1 turns=2000 tool_lines=24
    viewport: 160x48
    transcript: messages=12000 rendered_lines=80727 relevant_items=12000 replay_elapsed_ms=144.25
    full_layout:   110.53 ms
    full_draw:     0.88 ms
    scroll_draw:   p50 0.76 p95 0.80 avg 0.76 max 0.80 ms
    typing_draw:   p50 0.71 p95 0.74 avg 0.72 max 0.81 ms
    append_total:  p50 0.76 p95 0.83 avg 0.77 max 0.83 ms

Current real-session evidence after the incremental cache:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=139591 rendered_lines=4078383 relevant_items=139591 replay_elapsed_ms=5982.63
    full_layout:   4348.32 ms
    full_draw:     0.71 ms
    scroll_draw:   p50 0.59 p95 0.66 avg 0.60 max 0.70 ms
    typing_draw:   p50 0.61 p95 0.64 avg 0.61 max 0.66 ms
    append_total:  p50 0.62 p95 0.67 avg 0.62 max 0.67 ms

Current synthetic evidence after the block-backed redraw path and animation scheduling cleanup:

    target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48
    carlos perf-session
    source: synthetic seed=1 turns=2000 tool_lines=24
    viewport: 160x48
    transcript: messages=12000 rendered_lines=80727 relevant_items=12000 replay_elapsed_ms=138.75
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 31.74 p95 31.74 avg 31.74 max 31.74 ms
    full_layout:   105.67 ms
    full_draw:     0.89 ms
    scroll_draw:   p50 0.79 p95 0.81 avg 0.79 max 0.82 ms
    typing_draw:   p50 0.75 p95 0.76 avg 0.75 max 0.84 ms
    working_draw:  p50 0.74 p95 0.75 avg 0.75 max 0.83 ms
    append_total:  p50 0.77 p95 0.87 avg 0.78 max 0.87 ms

Current real-session evidence after the block-backed redraw path and animation scheduling cleanup:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=139719 rendered_lines=4079728 relevant_items=139719 replay_elapsed_ms=5969.65
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.30 ms
    full_layout:   4326.11 ms
    full_draw:     0.67 ms
    scroll_draw:   p50 0.60 p95 0.68 avg 0.60 max 0.72 ms
    typing_draw:   p50 0.58 p95 0.59 avg 0.58 max 0.63 ms
    working_draw:  p50 0.58 p95 0.59 avg 0.58 max 0.63 ms
    append_total:  p50 0.59 p95 0.62 avg 0.59 max 0.62 ms

Current synthetic evidence after the lazy block-materialization prepass:

    target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48
    carlos perf-session
    source: synthetic seed=1 turns=2000 tool_lines=24
    viewport: 160x48
    transcript: messages=12000 rendered_lines=80727 relevant_items=12000 replay_elapsed_ms=92.05
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 29.54 p95 29.54 avg 29.54 max 29.54 ms
    full_layout:   60.83 ms
    full_draw:     1.13 ms
    scroll_draw:   p50 0.77 p95 0.80 avg 0.77 max 0.92 ms
    typing_draw:   p50 0.67 p95 0.69 avg 0.66 max 0.69 ms
    working_draw:  p50 0.67 p95 0.69 avg 0.67 max 0.72 ms
    append_total:  p50 0.72 p95 0.77 avg 0.72 max 0.77 ms

Current real-session evidence after the lazy block-materialization prepass:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=139816 rendered_lines=4082463 relevant_items=139816 replay_elapsed_ms=4418.51
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.27 ms
    full_layout:   2828.81 ms
    full_draw:     0.70 ms
    scroll_draw:   p50 0.69 p95 1.48 avg 0.79 max 2.84 ms
    typing_draw:   p50 0.60 p95 0.60 avg 0.60 max 0.65 ms
    working_draw:  p50 0.60 p95 0.61 avg 0.60 max 0.65 ms
    append_total:  p50 0.60 p95 0.67 avg 0.61 max 0.67 ms

## Interfaces and Dependencies

Keep using the existing Rust stack and data model. The important interfaces for this ExecPlan
are:

- `src/app/state.rs`
  - `AppState::ensure_rendered_lines`
  - `AppState::mark_transcript_dirty`
  - any new dirty-range or cached-block helpers introduced to replace the current boolean dirty
    flag
- `src/app/render.rs`
  - `build_rendered_lines_with_hidden`
  - helpers that wrap plain, markdown, ANSI, and diff content into `RenderedLine`
- `src/app/input.rs`
  - the main TUI loop and draw cadence
- `src/app/perf_session.rs`
  - the offline perf harness that must continue to benchmark the real hot paths
- `src/tests.rs`
  - targeted correctness and perf-regression tests

Revision note: updated on 2026-03-24 to record the lazy block-materialization prepass, the
reduced full-layout cost on the large captured session, and the remaining follow-up work on the
count prepass itself.
