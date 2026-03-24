# autonomous execution prompt

Read `AGENTS.md`, `PROGRAM_PLAN.md`, and `.agents/PLANS.md` to orient yourself.
Identify the next incomplete ExecPlan from `PROGRAM_PLAN.md` and begin or continue execution.

## rules

- Work autonomously per `AGENTS.md`. Do not stop for routine confirmation.
- Keep the active ExecPlan current as you make progress.
- Commit frequently with small, buildable commits using scoped imperative subjects.
- Run `cargo test` after meaningful changes.
- When changing installed runtime behavior, also run `cargo build --release` and install the
  release binary to `~/.local/bin/carlos`.
- For performance work, use `carlos perf-session` with a real session or `--synthetic` to
  capture evidence instead of relying only on subjective feel.

## blocking protocol

If you hit a hard blocking condition from `AGENTS.md`, do the following:

1. State the blocker clearly in your response.
2. State what information or decision is needed to continue.
3. Output exactly this marker on its own line: `@@BLOCKED@@`
4. Stop execution.

## completion protocol

When every ExecPlan in `PROGRAM_PLAN.md` is complete and moved to `.agents/done/`:

1. Output exactly this marker on its own line: `@@COMPLETE@@`
2. Then provide a short narrative handoff describing what was completed, what was learned, and
   any remaining risks or follow-up context.
3. Stop execution.

## continuation

If you were interrupted and not blocked, continue from the current plan state without asking what
to do next.
