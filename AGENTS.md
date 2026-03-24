# AGENTS.md

This file defines the repository-level operating model for `carlos`.

## 0. Operating Modes

There are two modes:

- `Discussion Mode`: the default interactive mode where a human asks questions, requests
  changes, or steers the work directly.
- `Ralph Mode`: an autonomous execution mode entered when the session is started with
  `.agents/ralph-prompt.md` and the agent is expected to keep working through
  `PROGRAM_PLAN.md` plus the active ExecPlan.

Sections 1 through 6 apply in Ralph Mode. Section 7 applies in both modes.

## 1. Ralph Execution Model

Non-trivial Ralph work must run under `PROGRAM_PLAN.md` plus one or more ExecPlans.

- `PROGRAM_PLAN.md` is the coordination document. It states the overall goal, the active
  ExecPlans, and the repo-wide invariants.
- ExecPlans live under `.agents/execplans/`.
- Every active ExecPlan is a living document and must keep these sections current:
  `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`.

Rules:

- Ralph does not start substantial work without an ExecPlan.
- Ralph reads `PROGRAM_PLAN.md`, finds the next incomplete ExecPlan, and executes it.
- Completed ExecPlans are moved unchanged to `.agents/done/`.

## 2. Continuous Execution

In Ralph Mode, the agent keeps working until the active plan reaches a terminal state.

- Do not stop for routine confirmation.
- Do not ask what to do next after every milestone.
- Keep the plan documents current as work progresses.
- Prefer small, buildable commits that leave the tree usable.

## 3. Hard Blocking Conditions

Ralph may stop only for a real blocker. Valid blockers are:

1. Required external input is missing and cannot be inferred safely.
2. Two authoritative instructions conflict and the conflict changes the implementation.
3. The ExecPlan is underspecified enough that multiple incompatible implementations would all
   look reasonable.
4. Continuing would violate a stated invariant from `PROGRAM_PLAN.md` or another repo-level
   decision document.

If blocked, Ralph must:

1. Explain the blocker clearly.
2. State what information or decision is needed.
3. Output `@@BLOCKED@@` on its own line.
4. Stop until the human responds.

No other pauses are allowed.

## 4. Progress Reporting

Progress reporting must stay resumable.

- Keep the active ExecPlan current.
- Emit short in-band updates during long runs so a resumed session can recover context.
- Record concrete discoveries, changed assumptions, and the next action.
- Avoid generic narration that does not help future resumption.

## 5. Review Model

Use two reviewer roles for non-trivial changes:

- `Spec Reviewer`: checks alignment with the provided requirements, plans, and stated
  invariants. Prompt file: `.agents/spec_reviewer.md`
- `Engineering Reviewer`: checks correctness, edge cases, safety, maintainability, and test
  coverage. Prompt file: `.agents/engineering_reviewer.md`

Reviews should run in separate sessions. Their output should be copied into the active ExecPlan
under a clearly labeled review section.

Required outcomes:

- Spec reviewer: `APPROVED`, `APPROVED WITH NOTES`, or `REJECTED`
- Engineering reviewer: `PASS`, `PASS WITH ISSUES`, or `FAIL`

`REJECTED` and `FAIL` block forward progress until resolved.

## 6. Carlos Repository Discipline

These rules apply in Ralph Mode for this repository:

- Work on `main` unless the repository explicitly requires another flow.
- Use scoped imperative commit messages in the form `<scope>: <verb phrase>`.
- Prefer small, reviewable commits.
- Keep the repository buildable unless the active ExecPlan explicitly justifies an
  intermediate non-buildable state.
- Run `cargo test` after meaningful code changes.
- When changing the installed runtime behavior of `carlos`, also run `cargo build --release`
  and install the release binary to `~/.local/bin/carlos`.
- For transcript or performance work, use `carlos perf-session ...` with either a captured
  session file or `--synthetic` to measure regressions instead of relying on subjective feel
  alone.
- Do not silently weaken invariants or erase important context from the plan documents.

## 7. App-Server Schema Extraction

Use the Codex CLI generator to refresh the local protocol schema bundle:

```bash
mkdir -p docs/app-server-schema
codex app-server generate-json-schema --experimental --out docs/app-server-schema
```

Optional TypeScript bindings snapshot:

```bash
mkdir -p docs/app-server-ts
codex app-server generate-ts --experimental --out docs/app-server-ts
```

Key files to inspect after generation:

1. `docs/app-server-schema/codex_app_server_protocol.schemas.json` (full bundled schema)
2. `docs/app-server-schema/ServerNotification.json` (notification method union)
3. `docs/app-server-schema/v2/ThreadTokenUsageUpdatedNotification.json` (token usage shape)
4. `docs/app-server-schema/v2/ContextCompactedNotification.json` (legacy compaction notification)

Current context indicator in `carlos` depends on `thread/tokenUsage/updated` payload fields:

1. `params.tokenUsage.modelContextWindow` (max context window)
2. `params.tokenUsage.total.totalTokens` (used tokens)
