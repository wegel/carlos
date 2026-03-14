# PROGRAM_PLAN.md

This file coordinates Ralph-mode execution for the repository.

Update it when you add, reorder, or retire ExecPlans. It should answer three questions for a
resuming agent:

1. What is the overall goal?
2. Which ExecPlan should be worked on next?
3. What global invariants must not be violated while implementing those plans?

## Goal

Replace this paragraph with the project-level outcome Ralph is trying to achieve. Focus on
observable behavior, not just code movement.

## Global Invariants

- Keep the repository buildable and the default test command green after meaningful changes.
- Every non-trivial change must belong to an ExecPlan.
- Move completed ExecPlans to `.agents/done/` without editing them after completion.

## Active ExecPlans

- [ ] `.agents/execplans/EXECPLAN_001_example.md`

## Done ExecPlans

- None yet.
