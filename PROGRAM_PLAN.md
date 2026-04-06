# PROGRAM_PLAN.md

This file coordinates Ralph-mode execution for the `carlos` repository.

## Goal

Keep `carlos` smooth and responsive on very large sessions while also making the runtime easier
to evolve safely. The user-visible goal remains fast scrolling, typing, and live transcript
updates even on very large sessions, but the codebase goal is now to preserve that performance
while reducing the architectural risk caused by oversized, mixed-responsibility runtime files.
The next feature goal is to add a Claude Code backend without regressing the existing Codex path
or weakening the runtime discipline established by the earlier ExecPlans.

## Global Invariants

- Keep the repository buildable and `cargo test` green after meaningful changes.
- Every non-trivial Ralph change must belong to an ExecPlan.
- Use the perf harness to measure large-session regressions instead of relying only on visual
  judgment.
- Do not regress transcript fidelity, selection/copy behavior, reasoning/tool rendering, or
  approval handling while optimizing performance.
- Do not regress the existing Codex backend while adding alternative backend support.
- Move completed ExecPlans to `.agents/done/` without editing them after completion.

## Active ExecPlans


## Done ExecPlans

- [x] `.agents/done/EXECPLAN_001_large_session_smoothness.md`
- [x] `.agents/done/EXECPLAN_002_runtime_modularization.md`
- [x] `.agents/done/EXECPLAN_003_structural_hygiene_followup.md`
- [x] `.agents/done/EXECPLAN_004_claude_backend.md`
- [x] `.agents/done/EXECPLAN_005_claude_resume_history_import.md`
