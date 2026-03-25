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
- [x] (2026-03-24 02:40Z) Replaced temporary wrap allocation in the count prepass with count-only
  helpers and fast paths for common reasoning/ANSI cases.
- [x] (2026-03-24 03:05Z) Replaced diff-viewer rendering in the count prepass with direct hunk
  counting and stabilized the append benchmark so it always exercises a fresh plain assistant
  tail message.
- [x] (2026-03-24 03:20Z) Added a layout breakdown to `perf-session`, then used it to bypass ANSI
  handling entirely for tool outputs with no terminal escapes.
- [x] (2026-03-24 03:40Z) Added an ASCII single-line fast path in the natural wrap/count helpers
  so the dominant plain-text buckets can skip Unicode width work when a line already fits the
  viewport.
- [x] (2026-03-24 04:00Z) Added an ASCII multiline count fast path for non-user plain-text
  messages so giant tool-output blocks count fitting lines with a byte scan and only fall back to
  the slower wrapper for the smaller set of overflowing lines.
- [x] (2026-03-24 04:20Z) Added a per-layout cache for repeated long ASCII logical lines so the
  count prepass can reuse wrapped-line counts across recurring tool-output and tool-call text.
- [x] (2026-03-24 04:40Z) Replaced the slow count-only path’s wrapped-string allocation with the
  lower-level `textwrap` word/wrap pipeline plus the existing hard-wrap fallback for overlong
  tokens.
- [x] (2026-03-24 05:00Z) Added ASCII fast paths to the shared width/split helpers so the
  remaining plain-text-heavy hot paths stop paying grapheme/Unicode-width cost for ASCII-only
  text.
- [x] (2026-03-24 06:00Z) Ran engineering review on commits `5bcf8d3..f4b962e`, fixed the
  reported fenced-block count regression in the non-user ASCII fast path, and added a cached
  layout regression test for fence-delimited tool output.
- [ ] Shrink the initial full-layout cost further; it improved materially, but it is still about
  `1.21 s` on the captured 4M-line session.

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

- Observation: once the lazy-materialization split exists, ordinary wrap-count allocation is no
  longer the dominant remaining cost.
  Evidence: replacing temporary wrapped-string allocation with count-only helpers and adding fast
  paths for common reasoning and ANSI cases only moved the captured-session `full_layout` from
  `2828.81 ms` to `2802.99 ms`, which means the remaining time is likely in markdown/diff parsing
  and message-wide preprocessing rather than plain wrapping.

- Observation: diff counting was still a real hot spot because the count path was instantiating
  the full diff viewer just to measure rows.
  Evidence: the large captured session contains about `1696` diff-like tool outputs, and
  replacing viewer rendering with direct parsed-hunk counting reduced `full_layout` from
  `2802.99 ms` to `2331.66 ms` while keeping `append_total` at `0.67 ms` once the harness was
  normalized to use a fresh plain tail message.

- Observation: the remaining bottleneck after diff counting is overwhelmingly tool-output line
  counting, and most of those outputs are plain text rather than ANSI-colored output.
  Evidence: the new `layout_breakdown` shows `tool_output_ansi` at `1398.66 ms` after the diff
  fast path, while a raw session scan found `0` escaped outputs among `56,901` raw tool-output
  items. Adding a no-escape fast path reduced the `tool_output_ansi` bucket to `1104.24 ms` and
  the total `full_layout` to `1526.25 ms` on the latest validated run.

- Observation: on the current captured session, the dominant tool-output bucket is not just
  plain text, it is usually ASCII text whose logical lines already fit the viewport.
  Evidence: a raw scan of the current session file found `3,331,768` out of `3,444,461`
  tool-output logical lines at or under `160` columns, with `3,427,329` ASCII lines overall.
  Adding an ASCII single-line fast path reduced the real-session `tool_output_ansi` bucket from
  `1104.24 ms` to `1004.13 ms` and the total `full_layout` to `1391.56 ms`.

- Observation: after the single-line fast path, the remaining plain-text cost was still inflated
  by calling the generic wrapper millions of times just to rediscover that most ASCII lines fit.
  Evidence: adding a byte-scan multiline fast path for non-user plain text reduced the same
  real-session `tool_output_ansi` bucket from `1004.13 ms` to `947.85 ms`, `tool_call_plain`
  from `214.53 ms` to `203.09 ms`, and the total `full_layout` to `1341.73 ms`.

- Observation: repeated boilerplate lines are common enough that memoizing long-line counts within
  a single layout pass is worthwhile.
  Evidence: a raw scan of the captured session found about `3.45M` tool-output logical lines but
  only about `694k` unique lines, with extremely common repeats such as the empty line, `Output:`,
  and `Process exited with code 0`. Adding a per-layout cache for long ASCII line counts reduced
  the real-session `tool_output_ansi` bucket from `947.85 ms` to `903.75 ms` and the total
  `full_layout` to `1300.34 ms`.

- Observation: even after the cheap ASCII fast paths and repeated-line cache, the remaining slow
  path was still paying to allocate wrapped strings just to count them.
  Evidence: replacing that count-only slow path with the lower-level `textwrap` word separation
  plus wrap-algorithm pipeline reduced the real-session `tool_output_ansi` bucket from
  `903.75 ms` to `838.22 ms`, `tool_call_plain` from `205.34 ms` to `189.13 ms`, and the total
  `full_layout` to `1222.98 ms` on the current snapshot.

- Observation: the shared width helpers were still doing full grapheme/Unicode-width work for the
  overwhelmingly ASCII transcript hot path.
  Evidence: adding ASCII fast paths to `visual_width()` and `split_at_cells()` reduced the current
  real-session `tool_output_ansi` bucket from `838.22 ms` to `743.55 ms`, `tool_call_plain` from
  `189.13 ms` to `171.42 ms`, and the total `full_layout` to `1101.24 ms` while keeping
  `typing_draw`, `working_draw`, and `append_total` around `0.58 ms`.

- Observation: the line-count fast paths must preserve every delimiter rule that the materialized
  renderer applies, or the cached block offsets become incorrect even when the visible lines still
  render correctly.
  Evidence: engineering review found that the non-user ASCII multiline fast path skipped
  fence-delimiter handling, which caused cached line counts to disagree with rendered blocks for
  fenced tool output. Tightening the shortcut to bail out only when a real fence-delimiter line is
  present restored correctness and kept the large-session `full_layout` close to the pre-review
  snapshot at about `1.21 s`.

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

The work is still in progress, but the interactive path is now firmly in the target range and the
remaining cost is concentrated in the one-time full-layout prepass. On the captured 4M-line
session, `append_total`, `scroll_draw`, `typing_draw`, and `working_draw` are all around
`0.6–0.8 ms`, so live typing, scrolling, and the animation are no longer the problem. The main
remaining risk is the `~1.21 s` initial full-layout cost on the largest histories, plus the still
pending spec review required by the repo process before this ExecPlan can be closed.

## Reviews

### Engineering Review

- Reviewer: separate reviewer session using `.agents/engineering_reviewer.md`
- Scope: commits `5bcf8d3..f4b962e` plus this ExecPlan
- Verdict: `PASS WITH ISSUES`
- Finding resolved: the reviewer identified a `MAJOR` correctness regression in the non-user ASCII
  line-count fast path, where fence-delimited tool/commentary text could count fewer lines than it
  rendered. The fix narrows the shortcut to skip only texts without actual fence-delimiter lines,
  and `ensure_rendered_lines_non_user_fence_counts_match_rendered_block` now covers the cached
  layout path.
- Remaining review status: engineering review is satisfied after the fence-count fix. Spec review
  is still pending explicit authorization.

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

Current real-session evidence after count-only wrap helpers and common-case fast paths:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=139856 rendered_lines=4122766 relevant_items=139856 replay_elapsed_ms=4396.60
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.28 ms
    full_layout:   2802.99 ms
    full_draw:     0.72 ms
    scroll_draw:   p50 0.70 p95 2.08 avg 0.88 max 2.86 ms
    typing_draw:   p50 0.54 p95 0.56 avg 0.54 max 0.60 ms
    working_draw:  p50 0.54 p95 0.56 avg 0.54 max 0.58 ms
    append_total:  p50 0.70 p95 0.78 avg 0.71 max 0.78 ms

Current synthetic evidence after direct diff counting and a stabilized append benchmark:

    target/release/carlos perf-session --synthetic --turns 2000 --seed 1 --tool-lines 24 --width 160 --height 48
    carlos perf-session
    source: synthetic seed=1 turns=2000 tool_lines=24
    viewport: 160x48
    transcript: messages=12001 rendered_lines=80728 relevant_items=12000 replay_elapsed_ms=88.49
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 28.93 p95 28.93 avg 28.93 max 28.93 ms
    full_layout:   58.23 ms
    full_draw:     0.94 ms
    scroll_draw:   p50 0.79 p95 0.82 avg 0.79 max 0.84 ms
    typing_draw:   p50 0.68 p95 0.69 avg 0.68 max 0.73 ms
    working_draw:  p50 0.67 p95 0.69 avg 0.67 max 0.72 ms
    append_total:  p50 0.70 p95 0.74 avg 0.70 max 0.74 ms

Current real-session evidence after direct diff counting and a stabilized append benchmark:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140604 rendered_lines=4142439 relevant_items=140603 replay_elapsed_ms=3946.98
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.27 ms
    full_layout:   2331.66 ms
    full_draw:     0.72 ms
    scroll_draw:   p50 0.74 p95 1.75 avg 0.94 max 5.12 ms
    typing_draw:   p50 0.59 p95 0.60 avg 0.59 max 0.63 ms
    working_draw:  p50 0.57 p95 0.60 avg 0.58 max 0.64 ms
    append_total:  p50 0.67 p95 0.68 avg 0.66 max 0.68 ms

Current real-session evidence after the ANSI no-escape fast path:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140663 rendered_lines=4143376 relevant_items=140662 replay_elapsed_ms=4668.06
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.28 ms
    full_layout:   1526.25 ms
    full_draw:     0.67 ms
    scroll_draw:   p50 0.66 p95 1.81 avg 0.79 max 2.46 ms
    typing_draw:   p50 0.58 p95 0.58 avg 0.58 max 0.62 ms
    working_draw:  p50 0.58 p95 0.59 avg 0.58 max 0.60 ms
    append_total:  p50 0.60 p95 0.64 avg 0.60 max 0.64 ms
    layout_breakdown:
      tool_output_ansi msgs=48756 lines=3382453 total_ms=1104.24
      tool_call_plain msgs=50494 lines=323508 total_ms=221.28
      assistant_markdown msgs=1368 lines=34930 total_ms=108.70
      user_plain msgs=2555 lines=127825 total_ms=30.41
      reasoning_markdown msgs=33648 lines=73991 total_ms=25.24
      commentary_plain msgs=2119 lines=7307 total_ms=15.76
      diff msgs=1722 lines=193359 total_ms=14.43

Current real-session evidence after the ASCII single-line fast path:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140697 rendered_lines=4143676 relevant_items=140696 replay_elapsed_ms=4458.75
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.28 ms
    full_layout:   1391.56 ms
    full_draw:     0.62 ms
    scroll_draw:   p50 0.65 p95 2.12 avg 0.80 max 2.40 ms
    typing_draw:   p50 0.55 p95 0.57 avg 0.55 max 0.68 ms
    working_draw:  p50 0.55 p95 0.57 avg 0.55 max 0.58 ms
    append_total:  p50 0.59 p95 0.61 avg 0.59 max 0.61 ms
    layout_breakdown:
      tool_output_ansi msgs=48772 lines=3382702 total_ms=1004.13
      tool_call_plain msgs=50509 lines=323544 total_ms=214.53
      assistant_markdown msgs=1368 lines=34930 total_ms=109.98
      user_plain msgs=2555 lines=127825 total_ms=27.21
      reasoning_markdown msgs=33651 lines=74006 total_ms=24.18
      commentary_plain msgs=2119 lines=7307 total_ms=15.95
      diff msgs=1722 lines=193359 total_ms=14.60

Current real-session evidence after the ASCII multiline count fast path:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140723 rendered_lines=4148285 relevant_items=140722 replay_elapsed_ms=4329.90
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.32 ms
    full_layout:   1341.73 ms
    full_draw:     0.70 ms
    scroll_draw:   p50 0.69 p95 1.54 avg 0.80 max 2.64 ms
    typing_draw:   p50 0.59 p95 0.60 avg 0.59 max 0.63 ms
    working_draw:  p50 0.58 p95 0.60 avg 0.58 max 0.63 ms
    append_total:  p50 0.62 p95 0.65 avg 0.62 max 0.65 ms
    layout_breakdown:
      tool_output_ansi msgs=48783 lines=3387179 total_ms=947.85
      tool_call_plain msgs=50520 lines=323648 total_ms=203.09
      assistant_markdown msgs=1368 lines=34930 total_ms=107.34
      user_plain msgs=2555 lines=127825 total_ms=26.38
      reasoning_markdown msgs=33655 lines=74034 total_ms=23.79
      commentary_plain msgs=2119 lines=7307 total_ms=15.55
      diff msgs=1722 lines=193359 total_ms=14.35

Current real-session evidence after memoizing repeated long ASCII lines:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140777 rendered_lines=4148787 relevant_items=140776 replay_elapsed_ms=4254.17
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.32 ms
    full_layout:   1300.34 ms
    full_draw:     0.63 ms
    scroll_draw:   p50 0.67 p95 1.69 avg 0.79 max 2.45 ms
    typing_draw:   p50 0.56 p95 0.56 avg 0.56 max 0.59 ms
    working_draw:  p50 0.56 p95 0.57 avg 0.56 max 0.60 ms
    append_total:  p50 0.59 p95 0.62 avg 0.59 max 0.62 ms
    layout_breakdown:
      tool_output_ansi msgs=48803 lines=3387542 total_ms=903.75
      tool_call_plain msgs=50540 lines=323701 total_ms=205.34
      assistant_markdown msgs=1370 lines=34965 total_ms=110.52
      user_plain msgs=2561 lines=127843 total_ms=27.72
      reasoning_markdown msgs=33661 lines=74067 total_ms=24.88
      commentary_plain msgs=2119 lines=7307 total_ms=16.15
      diff msgs=1722 lines=193359 total_ms=14.66

Current real-session evidence after the low-level count-only wrap path:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140820 rendered_lines=4150554 relevant_items=140819 replay_elapsed_ms=4091.77
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.33 ms
    full_layout:   1222.98 ms
    full_draw:     0.82 ms
    scroll_draw:   p50 0.68 p95 1.68 avg 0.78 max 2.20 ms
    typing_draw:   p50 0.64 p95 0.65 avg 0.64 max 0.69 ms
    working_draw:  p50 0.64 p95 0.65 avg 0.64 max 0.68 ms
    append_total:  p50 0.67 p95 0.70 avg 0.67 max 0.70 ms
    layout_breakdown:
      tool_output_ansi msgs=48818 lines=3388656 total_ms=838.22
      tool_call_plain msgs=50556 lines=324189 total_ms=189.13
      assistant_markdown msgs=1370 lines=34965 total_ms=109.23
      user_plain msgs=2561 lines=127843 total_ms=25.57
      reasoning_markdown msgs=33672 lines=74163 total_ms=23.62
      commentary_plain msgs=2119 lines=7307 total_ms=15.02
      diff msgs=1723 lines=193428 total_ms=14.46

Current real-session evidence after the ASCII width-helper fast paths:

    target/release/carlos perf-session /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl --width 160 --height 48
    carlos perf-session
    source: /home/wegel/.codex/sessions/2026/02/15/rollout-2026-02-15T18-18-49-019c6286-d480-7293-8fd8-bd6459fab3ad.jsonl
    viewport: 160x48
    transcript: messages=140844 rendered_lines=4151080 relevant_items=140843 replay_elapsed_ms=3837.92
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.00 p95 0.01 avg 0.00 max 0.28 ms
    full_layout:   1101.24 ms
    full_draw:     0.64 ms
    scroll_draw:   p50 0.65 p95 1.15 avg 0.70 max 1.35 ms
    typing_draw:   p50 0.57 p95 0.58 avg 0.57 max 0.62 ms
    working_draw:  p50 0.57 p95 0.59 avg 0.57 max 0.61 ms
    append_total:  p50 0.58 p95 0.61 avg 0.58 max 0.61 ms
    layout_breakdown:
      tool_output_ansi msgs=48829 lines=3388987 total_ms=743.55
      tool_call_plain msgs=50568 lines=324379 total_ms=171.42
      assistant_markdown msgs=1370 lines=34965 total_ms=107.61
      user_plain msgs=2561 lines=127843 total_ms=23.24
      reasoning_markdown msgs=33673 lines=74168 total_ms=22.95
      commentary_plain msgs=2119 lines=7307 total_ms=14.50
      diff msgs=1723 lines=193428 total_ms=14.49

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
follow-up count-only helper passes, the diff-count fast path, the ANSI no-escape fast path, the
ASCII single-line fast path, the ASCII multiline count fast path, the repeated-line memoization
pass, the low-level count-only wrap path, the ASCII width-helper fast paths, the measured
reductions in full-layout cost on the large captured session, the `perf-session` layout
breakdown, and the evidence that the remaining bottleneck is now mostly plain tool-output line
counting.
