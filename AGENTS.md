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

Use the reviewer prompts present under `.agents/reviewers/` for non-trivial changes.

- Each reviewer prompt file defines its role, review scope, and expected verdicts.
- If more than one reviewer prompt is present, run each applicable reviewer in a separate
  session.
- If only one reviewer prompt is present, run that reviewer.
- Persist reviewer session ids under `.agents/reviewer_sessions.json`. Record at least the
  reviewer prompt path, session id, status, and the latest reviewed change so later Ralph runs
  can resume cleanly.
- When a reviewer session already exists for a reviewer, always resume and reuse that existing
  session instead of starting a new one.
- Only create a new reviewer session when no stored session exists for that reviewer, or when
  the stored session can no longer be resumed. In that case, replace the stored session id with
  the new one.
- The first message in a newly created reviewer session must be the reviewer prompt itself.
- Supply only the concrete review subject as additional context: the change range, change
  intent, validation/perf evidence, and any invariants the reviewer should check.
- Do not wrap reviewer invocation in extra process narration, Ralph workflow instructions, or
  blocker language unless the reviewer prompt explicitly requires it.
- A reviewer session that is still exploring files or narrating interim reasoning is not blocked
  just because it has not produced a verdict yet. Let it continue, do other work if possible,
  and resume the same session later.
- When a reviewer session needs to produce a capturable final verdict, reattach to that same
  session in an inline or no-alt-screen mode so the verdict text is visible in logs instead of
  hidden behind a terminal UI buffer.
- If a reviewer responds without following its required output shape, treat that as a bad
  invocation or prompt issue. First ask the same reviewer session for the required output shape
  only. If that still fails, retry once with a stricter role-preserving invocation before
  treating review as blocked.

Reviews should run in separate sessions. Their output should be copied into the active ExecPlan
under a clearly labeled review section.

Any reviewer verdict that is marked blocking in its prompt file blocks forward progress until
resolved.

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
