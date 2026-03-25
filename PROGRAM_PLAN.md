# PROGRAM_PLAN.md

This file coordinates Ralph-mode execution for the `carlos` repository.

## Goal

Make `carlos` stay smooth and responsive on very large sessions while preserving transcript
correctness, copy behavior, model/tool visibility, and the existing TUI feature set. The main
user-visible goal is that scrolling, typing, and live transcript updates stay fast even when a
session contains tens of thousands of rendered lines.

## Global Invariants

- Keep the repository buildable and `cargo test` green after meaningful changes.
- Every non-trivial Ralph change must belong to an ExecPlan.
- Use the perf harness to measure large-session regressions instead of relying only on visual
  judgment.
- Do not regress transcript fidelity, selection/copy behavior, reasoning/tool rendering, or
  approval handling while optimizing performance.
- Move completed ExecPlans to `.agents/done/` without editing them after completion.

## Active ExecPlans

- None currently.

## Done ExecPlans

- [x] `.agents/done/EXECPLAN_001_large_session_smoothness.md`
