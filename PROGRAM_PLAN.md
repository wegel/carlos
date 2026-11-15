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

EP018a is complete and archived. The owner accepted it on 2026-11-13 after reviewing
`.agents/reports/exec018a-final-report.md` and the independent whole-roadmap follow-up in
`.agents/reports/exec018a-fable-followup-review.md`. Its archived plan remains unchanged.
That acceptance does not authorize installing, merging or pushing a spike binary.

EP019a is complete and archived at `.agents/done/EXECPLAN_019a_headless_daemon.md`.
Its local `carlosd` proves real backend handles, concurrent headless clients, ordered sends,
responsive interrupts/answers, bounded queues and disconnect/restart recovery. Final evidence
and the direct engineering self-review are in `.agents/reports/exec019a-final-report.md` and
`.agents/reports/exec019a-engineering-review.md`. The owner explicitly accepted this step
on 2026-11-14 in the current conversation. Its archived plan remains unchanged.

EP019b is complete and archived at `.agents/done/EXECPLAN_019b_terminal_client.md`.
Root approved it under the owner's delegated grant on 2026-11-15 at 21:27 UTC after
complete final checks, exact-pair measurements, direct engineering PASS and independent
Fable 5.2 medium PASS WITH ISSUES. The final report and every finding/decision are in
`.agents/reports/exec019b-final-report.md` and `exec019b-review-disposition.md`.
Commit `cc2a5f5` separately removes the embedded runtime; final review corrections are
`cbdca8f`, runtime source `23d6e70775c623b8106074957efb103d`.

The next plan to write and execute is EP020. It must first add explicit typed session
stop/reopen and prove reuse of the 16 owner slots while preserving saved conversations,
private drafts and explicit retry choices. Only then add WebSocket/device-token access.
Protocol version 1 remains provisional. Keep main and the installed binary unchanged.

ExecPlans 019a through 022 are listed in `docs/engine-architecture.md`. Only the next plan
is written in full. On 2026-11-14, the owner delegated approval of completed EPs to Codex and asked it to continue through the next EPs while the owner sleeps. Before each auto-approval, also obtain a Fable 5.2 medium review of the code and whole roadmap, as the owner requested in the same conversation. Use judgment to address or explain each finding, then record the auto-approval and continue after all checks and the direct engineering review pass. Do not stop at the former per-EP human gate. On 2026-11-14, the owner further directed: no more reviewers until the end of EP019b. Continue implementation and tests without new slice or follow-up reviewer sessions; obtain the final engineering and Fable reviews when the whole EP is ready. This instruction supersedes the repository's earlier per-slice review cadence, not its correctness checks. This grant does not authorize merging, pushing or replacing the installed spike binary; keep those actions separate.

The final EP019b Fable review found that live owners remain allocated until daemon shutdown and closed non-Codex sessions need an explicit recovery command. EP020 must first add a typed session stop/reopen path, with actual multi-client/native tests, before enabling its network listener. Stopping a session releases its owner and native processes while keeping the saved conversation. This is a required first milestone, not optional later cleanup. The review remains `.agents/reports/exec019b-fable-final-review.md`; its final EP019b dispositions belong in the final report.

ExecPlans 009 to 015 remain under `.agents/execplans/`. Their status was not reviewed when the
daemon split started. They remain outside the iPhone spike; its current step is EP019b.


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
- [x] `.agents/done/EXECPLAN_017_core_workspace.md` (step 2 of 7: terminal-free session library; completed 2026-11-13)
- [x] `.agents/done/EXECPLAN_018_sequenced_events.md` (step 3 of 7: numbered core events and client protocol version 1; completed 2026-11-13)

- [x] `.agents/done/EXECPLAN_018a_pre_daemon_cleanup.md` (owner accepted 2026-11-13; archived plan unchanged)
- [x] `.agents/done/EXECPLAN_019a_headless_daemon.md` (completed and owner accepted 2026-11-14; archived plan unchanged)

- [x] `.agents/done/EXECPLAN_019b_terminal_client.md` (completed and approved under delegated authority 2026-11-15; final checks and whole-roadmap review pass)
