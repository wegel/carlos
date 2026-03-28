# Tighten transcript ownership and split remaining broad modules

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

`EXECPLAN_002` removed the largest mixed-responsibility files, but it intentionally stopped short of a deeper ownership cleanup. The remaining architectural risk is no longer “everything lives in one god file”; it is that some important invariants still rely on conventions across multiple modules, and two broad domains remain oversized: transcript rendering and tool formatting/parsing. After this change, contributors should be able to modify transcript mutation, transcript rendering, and tool-command formatting without relying on broad `pub(super)` field access or editing one large catch-all module.

This is not a cosmetic line-count exercise. The goal is to convert the reviewer’s `PASS WITH ISSUES` residual concerns into narrower ownership boundaries while preserving the large-session performance and transcript fidelity work already achieved.

## Progress

- [x] (2026-03-28 16:20Z) Created the structural-hygiene follow-up ExecPlan and registered it in `PROGRAM_PLAN.md`.
- [x] (2026-03-28 16:43Z) Recorded the file-size baseline for this ExecPlan: `transcript_render.rs 1288`, `tools.rs 1179`, `ui_render_tests.rs 1169`, `state.rs 879`, `perf_session.rs 824`, with `notification_items.rs` still at `523`.
- [x] (2026-03-28 16:57Z) Introduced an explicit mapped-item mutation boundary in `AppState` and moved the ordinary `notification_items.rs` item-started/item-completed lifecycle off direct transcript/index mutation.
- [x] (2026-03-28 17:22Z) Split `src/app/transcript_render.rs` into orchestration plus `transcript_styles.rs` and `transcript_diff.rs`, keeping the public render/count surface stable while preserving the frozen-session perf baseline (`full_layout 48.43 ms`, `append_total p50 0.68 ms`).
- [x] (2026-03-28 17:41Z) Split `src/app/tools.rs` into a façade plus `tool_shell.rs` and `tool_diff.rs`, reducing `tools.rs` itself to `692` lines while preserving command/tool rendering behavior and frozen-session perf (`full_layout 48.00 ms`, `append_total p50 0.68 ms`).
- [x] (2026-03-28 17:49Z) Reduced the broad implicit test prelude in the maintenance-sensitive tool and notification test modules by replacing `use super::*;` with explicit imports, while intentionally leaving the much larger UI-render test module for a future pass to avoid busywork.
- [x] (2026-03-28 17:56Z) Re-ran final validation on exact `HEAD`: `cargo test` passed, `cargo build --release` passed, the installed binary was refreshed, and the frozen perf snapshot remained flat (`full_layout 47.59 ms`, `full_draw 1.06 ms`, `append_total p50 0.68 ms`).
- [x] (2026-03-28 18:10Z) Collected the required engineering review. Verdict: `PASS` with no findings.
- [ ] Move this ExecPlan to `.agents/done/` and close it out in `PROGRAM_PLAN.md`.

## Surprises & Discoveries

- The broad invariant-leak the reviewer called out was concentrated much more narrowly than the line counts suggested: the ordinary `item/started` and `item/completed` paths in `notification_items.rs` were the main place still reaching directly into `messages`, `agent_item_to_index`, and dirty/coalesce behavior.
- `transcript_render.rs` split cleanly only once it was treated as an orchestration layer with a stable outward surface. Keeping the block-building/counting entry points in place while moving styled-text and diff logic underneath avoided churn in render cache, perf harness, and tests.
- `tools.rs` had the same pattern as `transcript_render.rs`: the stable surface was the façade of high-level tool-item helpers, while the real split seams were shell/SSH/control handling and diff extraction underneath it.
- The test-prelude cleanup had a clear diminishing-return point. Converting the domain-heavy notification/tool modules paid off immediately, but the giant UI-render test module is large enough that forcing the same change there right now would be mostly churn rather than hygiene.
- Final exact-HEAD validation stayed within noise of the pre-close milestone runs, so the modularization work did not reopen the large-session performance concern that motivated the earlier EPs.

## Decision Log

- Decision: treat the remaining work as a new ExecPlan instead of reopening `EXECPLAN_002`.
  Rationale: `EXECPLAN_002` achieved its stated objective and was closed cleanly. The remaining work is a follow-up architectural tightening pass, not incomplete prior scope.
  Date/Author: 2026-03-28 / codex

- Decision: prioritize transcript/message mutation ownership before splitting `transcript_render.rs` or `tools.rs`.
  Rationale: the engineering review identified invariant leakage through broad `AppState` access as the most important remaining maintainability issue. Narrowing mutation ownership first reduces the risk that later module splits merely move the same hazard around.
  Date/Author: 2026-03-28 / codex

- Decision: narrow the mutation boundary first by routing mapped item lifecycle updates through explicit `AppState` helpers before attempting to privatize every transcript/index field.
  Rationale: this removes the highest-risk cross-module mutation patterns immediately while keeping the change set small and behavior-preserving. Full field privacy can follow later if it still provides value after the main splits.
  Date/Author: 2026-03-28 / codex

- Decision: split `transcript_render.rs` into `transcript_styles.rs` and `transcript_diff.rs` under a smaller orchestration module, instead of pushing block-counting/materialization into yet another top-level file first.
  Rationale: the stable API surface for the rest of the app is the transcript block builder/counter. Preserving that surface minimized collateral edits while isolating the real subdomains that contributors naturally search for: styled-text shaping and diff rendering.
  Date/Author: 2026-03-28 / codex

- Decision: split `tools.rs` into `tool_shell.rs` and `tool_diff.rs` underneath a smaller `tools.rs` façade instead of scattering tool-item formatting across multiple new top-level entry points.
  Rationale: the app and tests naturally depend on `tools.rs` as the tool-behavior surface. Preserving that entry point let the shell/SSH/control logic and diff extraction move into coherent modules without forcing wide call-site churn.
  Date/Author: 2026-03-28 / codex

- Decision: scope the test-prelude cleanup to the notification and tool test modules in this ExecPlan instead of rewriting every child test module away from `use super::*`.
  Rationale: those modules are the highest-payoff non-UI domains for explicit imports today. The giant UI-render suite would require a much larger churn-heavy rewrite for comparatively less architectural gain, which would drift into busywork.
  Date/Author: 2026-03-28 / codex

## Outcomes & Retrospective

- Outcome: the remaining non-pedantic hygiene work is complete. Transcript/item lifecycle updates now go through narrower `AppState` helpers, transcript rendering is split into orchestration plus styled-text and diff subdomains, tool behavior is split into façade plus shell/diff subdomains, and the most maintenance-sensitive non-UI test modules no longer depend on the broad implicit prelude.
- Outcome: the work stayed within noise on the frozen large-session benchmark. The structural cleanup did not reopen the responsiveness problem that motivated the earlier performance EPs.
- Follow-up context: the largest remaining broad runtime file is now `src/app/state.rs`, but the residual risk there is much lower than before because the highest-value invariant leakage and mixed-responsibility modules from this plan have been addressed. A future EP could keep tightening state encapsulation or further decompose the very large UI-render test module if that begins to create real maintenance pain.

## Review

### Engineering Review

Reviewer: `.agents/reviewers/engineering_reviewer.md`
Change range: `3d46bda..a0fa3f3`
Verdict: `PASS`

Summary:
- Reviewed the transcript/item mutation-boundary tightening, transcript-render split, tool-layer split, and targeted test import cleanup against the stated invariants, local validation, and frozen perf evidence.

Findings:
- No findings.

Corrective guidance:
- No corrective changes required.

## Context and Orientation

The current largest and/or riskiest remaining areas are:

- `src/app/transcript_render.rs`: still a broad transcript domain that mixes line counting, block building, markdown shaping, diff rendering, and some normalization helpers.
- `src/app/tools.rs`: still a broad tool domain that mixes command parsing/summarization, SSH rewriting, ANSI/control handling, diff extraction, and formatted tool rendering.
- `src/app/state.rs` plus the domain modules that use it: substate extraction happened, but important transcript/index/UI invariants still depend on direct `pub(super)` field mutation from modules like `notification_items.rs` and `input_events.rs`.
- `src/tests/*.rs`: the test split improved discoverability, but every child module still relies on `use super::*;`, which keeps import boundaries looser than ideal.

The engineering reviewer verdict on `EXECPLAN_002` was `PASS WITH ISSUES`. The main issue to address here is:

- transcript/index/substate invariants are still enforced by convention across modules rather than a narrow mutation API

This plan should address that issue first, then reduce the remaining oversized modules by real subdomain.

## Plan of Work

Start by tightening transcript/message ownership. The likely end state is not a full rewrite of `AppState`, but a narrower mutation surface for:

- appending/updating transcript messages
- updating agent item mappings and turn diff mappings
- marking transcript dirty and coalescing read summaries

The important constraint is that callers such as `notification_items.rs` and `input_events.rs` should no longer need to directly mutate transcript/index internals in order to do ordinary work.

After that, split `src/app/transcript_render.rs` by actual transcript-rendering subdomains. The expected seam is roughly:

- transcript block counting/materialization
- markdown and styled-text shaping helpers
- diff-specific rendering/counting helpers

Do not split merely by size. Each resulting module must have a responsibility boundary that a contributor would naturally search for.

Then split `src/app/tools.rs`. The likely seam is roughly:

- parsed shell/command summarization
- SSH/remote-command extraction
- ANSI/control sanitization or ANSI-to-styled conversion
- tool item formatting and diff extraction

Again, avoid a “misc helpers” dump. The modules should reflect user-visible tool behaviors.

Finally, improve the new test modules only where the payoff is real. The target is not “every test imports everything explicitly right now”; it is to reduce the most brittle reliance on the giant implicit prelude and leave the test structure healthier than it is today.

## Milestones

### Milestone 1: Narrow transcript mutation ownership

At the end of this milestone, ordinary transcript/item lifecycle code should go through explicit mutation helpers instead of directly mutating a wide set of `AppState` fields. Acceptance is that transcript behavior remains unchanged, tests stay green, and the invariant surface is visibly narrower in code review.

### Milestone 2: Split `transcript_render.rs`

At the end of this milestone, transcript rendering no longer lives in one 1200+ line file. Acceptance is unchanged transcript fidelity and no meaningful perf regression in `carlos perf-session` on the frozen snapshot.

### Milestone 3: Split `tools.rs`

At the end of this milestone, tool formatting/parsing/rendering logic is grouped by coherent behavior rather than one large catch-all file. Acceptance is unchanged command/tool rendering behavior and green tests.

### Milestone 4: Tighten test-module boundaries

At the end of this milestone, the tests remain responsibility-grouped but with less dependence on a giant implicit prelude where explicit imports materially improve clarity. Acceptance is green tests and a cleaner module surface.

## Concrete Steps

All commands below should be run from the repository root: `/var/home/wegel/work/perso/carlos`.

1. Record the starting file-size baseline:

       find src -type f -name '*.rs' -print0 | xargs -0 wc -l | sort -nr | head -n 15

2. Before each milestone, identify the functions and types that will move:

       rg -n '^pub\\(super\\)? fn |^fn |^impl ' src/app/state.rs src/app/transcript_render.rs src/app/tools.rs

3. After each meaningful change, run:

       cargo test

4. After each runtime-affecting slice, run:

       cargo build --release
       cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

5. After each runtime-affecting milestone, run at least one frozen-session perf check:

       target/release/carlos perf-session /tmp/carlos-perf-session-019cdf51-snapshot.jsonl --width 160 --height 48

6. Keep this ExecPlan current after each slice by updating `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`.

## Validation and Acceptance

The minimum acceptance bar is:

- `cargo test` passes after each meaningful change.
- `cargo build --release` passes after each runtime-affecting milestone.
- The installed `~/.local/bin/carlos` binary is updated after runtime-affecting milestones.
- `carlos perf-session` shows no meaningful regression on the frozen snapshot relative to the current baseline.
- No regression in transcript fidelity, selection/copy behavior, tool rendering, reasoning rendering, or approval/Ralph flows.
- The final engineering review is not `FAIL`.

Final acceptance for this ExecPlan is not “the files got smaller.” Final acceptance is that transcript mutation invariants are less implicit, the remaining broad modules have been split by real domain boundaries, and the runtime remains measurably smooth and behaviorally stable.

## Idempotence and Recovery

This work should proceed in small, buildable commits. If a prospective split reveals no real boundary, stop and record that discovery rather than forcing a mechanical extraction. If tightening ownership causes API churn without reducing invariants leakage, back up and take a narrower seam.

Use the temporary-file-and-rename pattern for binary installation:

    cp target/release/carlos ~/.local/bin/carlos.new && mv ~/.local/bin/carlos.new ~/.local/bin/carlos

## Artifacts and Notes

- Pending: baseline file-size report for this ExecPlan.
- Current frozen perf baseline from the end of `EXECPLAN_002` is roughly `full_layout ~49 ms`, `full_draw ~0.78 ms`, and `append_total p50 ~0.69 ms` on `/tmp/carlos-perf-session-019cdf51-snapshot.jsonl`. Refresh this with an explicit measurement after the first implementation slice.
