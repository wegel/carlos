# PROGRAM_PLAN.md

This file coordinates Ralph-mode execution for the `carlos` repository.

## Goal

Keep `carlos` smooth and responsive on very large sessions while also making the runtime easier
to evolve safely. The user-visible goal remains fast scrolling, typing, and live transcript
updates even on very large sessions, but the codebase goal is now to preserve that performance
while reducing the architectural risk caused by oversized, mixed-responsibility runtime files.
The next feature goal is to finish the Claude Code integration so Claude-native approval flows,
starting with `ExitPlanMode`, work inside `carlos` without regressing the existing Codex path or
weakening the runtime discipline established by the earlier ExecPlans. The current feature goal is
to add optional in-process voice dictation to the prompt input box without blocking the TUI or
making audio and Whisper dependencies mandatory for users who do not need dictation.

The current architecture goal is to split `carlos` into a daemon (`carlosd`) that owns the
sessions and several clients: the terminal UI, phone apps, a web app and later a desktop app.
`docs/engine-architecture.md` holds the design, the wire protocol draft, the order of work and
the reasons. Read it before any work on ExecPlan 016 or later.

Keep ExecPlans 016 through 022 on branch `iphone-spike`, in the existing checkout
`/var/home/wegel/work/perso/carlos-daemon`. This branch was renamed from `daemon-split`
after ExecPlan 016. Do not merge it into `main` or push it without the owner's request.

## Global Invariants

- Keep the repository buildable and `cargo test` green after meaningful changes.
- Every non-trivial Ralph change must belong to an ExecPlan.
- Use the perf harness to measure large-session regressions instead of relying only on visual
  judgment.
- Do not regress transcript fidelity, selection/copy behavior, reasoning/tool rendering, or
  approval handling while optimizing performance.
- Do not regress the existing Codex backend while adding alternative backend support.
- Move completed ExecPlans to `.agents/done/` without editing them after completion.
- Carlos must never feel slow or bloated. For the daemon split, follow the speed rules in
  `docs/engine-architecture.md` and compare `carlos perf-session` figures with the baseline
  recorded in ExecPlan 016.
- Code that will move to `carlos-core` must not gain new references to terminal crates.

## Active ExecPlans

- [ ] `.agents/execplans/EXECPLAN_017_core_workspace.md` (step 2 of 7: build the session core without terminal libraries)

ExecPlans 017 to 022 are listed in `docs/engine-architecture.md`. Each one is written when the
plan before it ends.

ExecPlans 009 to 015 remain under `.agents/execplans/`. Their status was not reviewed when the
daemon split started.


## Done ExecPlans

- [x] `.agents/done/EXECPLAN_001_large_session_smoothness.md`
- [x] `.agents/done/EXECPLAN_002_runtime_modularization.md`
- [x] `.agents/done/EXECPLAN_003_structural_hygiene_followup.md`
- [x] `.agents/done/EXECPLAN_004_claude_backend.md`
- [x] `.agents/done/EXECPLAN_005_claude_resume_history_import.md`
- [x] `.agents/done/EXECPLAN_006_claude_exit_plan_mode.md`
- [x] `.agents/done/EXECPLAN_007_code_quality_bar.md`
- [x] `.agents/done/EXECPLAN_008_dictation.md`
- [x] `.agents/done/EXECPLAN_016_session_state_split.md` (step 1 of 7 of the daemon split; completed 2026-11-13)
