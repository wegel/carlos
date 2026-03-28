# PROGRAM_PLAN.md

This file coordinates Ralph-mode execution for the `carlos` repository.

## Goal

Keep `carlos` smooth and responsive on very large sessions while also making the runtime easier
to evolve safely. The user-visible goal remains fast scrolling, typing, and live transcript
updates even on very large sessions, but the codebase goal is now to preserve that performance
while reducing the architectural risk caused by oversized, mixed-responsibility runtime files.

## Global Invariants

- Keep the repository buildable and `cargo test` green after meaningful changes.
- Every non-trivial Ralph change must belong to an ExecPlan.
- Use the perf harness to measure large-session regressions instead of relying only on visual
  judgment.
- Do not regress transcript fidelity, selection/copy behavior, reasoning/tool rendering, or
  approval handling while optimizing performance.
- Move completed ExecPlans to `.agents/done/` without editing them after completion.

## Active ExecPlans

- None.

## Done ExecPlans

- [x] `.agents/done/EXECPLAN_001_large_session_smoothness.md`
- [x] `.agents/done/EXECPLAN_002_runtime_modularization.md`
