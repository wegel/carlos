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

EP020 is complete and archived at `.agents/done/EXECPLAN_020_network_access.md`.
Root approved it under the owner's delegated grant on 2026-11-16 at 04:31:57 UTC after
complete corrected gates, exact-pair measurements, direct engineering PASS WITH ISSUES,
whole-roadmap Fable PASS WITH ISSUES and focused Fable PASS WITH ISSUES with no remaining
BLOCKER or MAJOR finding. `.agents/reports/exec020-final-report.md` and
`exec020-review-disposition.md` retain all findings, costs, failed runs and decisions.
The final source is `6a31fd059ad6f8f0e7e92b1f001bc765`; the checked pair is frozen at
`/tmp/carlos-ep020/corrected-pair/identity.json`. The archived plan remains unchanged.

EP021a is complete and archived at `.agents/done/EXECPLAN_021a_shared_panel_reads.md`.
Root approved it under the owner's grant on2026-11-16 at11:34:22 UTC after the complete
corrected gate, fresh exact-pair measurements, direct engineering PASS WITH ISSUES and
Fable5.2 medium PASS WITH ISSUES. The final source is896435d22d03d85dba4cef0f58ba68e8,
narrow review correction06f13ce, checked tree3ffd3eb. Final evidence, every review decision
and the unchanged archive hash are recorded under `.agents/reports/exec021a-*`.
The checked baseline for the next plan is `/tmp/carlos-ep021a/reviewed-pair/identity.json`.

EP021b is complete and archived at `.agents/done/EXECPLAN_021b_native_phone_data.md`.
Root approved it under the owner's grant on2026-11-16 at20:33:35 UTC after corrected
full gate06, fresh exact-pair/native measurements, the single end-only Fable5.2 medium
PASS WITH ISSUES and Root's final engineering PASS. Runtime6de40a2, source
67a1a38a348a6343400594a6d06dbe4a and checked treefb5ed10 are frozen under
`/tmp/carlos-ep021b/reviewed/final-pair/identity.json`. Final reports and every finding,
failed/cancelled gate and decision are `.agents/reports/exec021b-*`.
The unchanged archive SHA256 is58df8613825d47d1e869b5940e9cf2368d6903fd7f2387a689acd533b18300bb.

The active next incomplete plan is `.agents/execplans/EXECPLAN_022_iphone_app.md`,
now written in full. Build the actual SwiftUI iPhone app with a narrow generated UniFFI
interface to carlos-client. Prove a small real Swift caller and owned simulator build
first, then implement the specified screens and real socket workflows. Keep Root direct,
no developer subagents, no intermediate reviewers and no routine human approval gate.
Read docs/ios-build.md and tmp/main-screen.md before writing app code. Use only the owned
Mac directory/simulator and leave its global Rust default and other projects untouched.

The first iPhone binding must follow the screens in tmp/main-screen.md section11,
rather than mirror every public Rust method. Export only session listing/lifecycle,
greeting/reconnect, transcript/history/source/output, prompt/interrupt/approval, context
display and Ralph calls those screens use. Queue, host context-default, goal and native
task-panel readers stay Rust-only until a phone screen needs them; the terminal uses them
already. A later web binding follows its actual screens too. EP021b records production
lines added/deleted per native feature and removes replaced paths. Correctness and complete
supported data take precedence over an arbitrary line quota. Phone-specific cache and ABI
error policies wait for the real EP022 facade. Version1 remains provisional.

Keep main, the installed binary and all frozen owner preview packages unchanged.
On2026-11-17 the owner explicitly requests using this branch as their normal local
Carlos and installing under~/.local/bin. That request authorizes a matching checked
CLI/daemon/helper install with backups; it does not authorize a daemon restart,
merge, push or changing frozen previews. The tool environment mounts~/.local/bin
read-only even after an escalated retry. Root prepared the checked package and a
normal-shell installer at tmp/install-carlos-92d3a535/install.py in the shared spike
checkout. Root subsequently installed the checked source284b3acc7aa71615fed2cef9509c316b
package after fixing oversized Codex history replies, with rollback files under
~/.local/lib/carlos/backups/20261117T203941Z. The owner resumed this conversation
inside Carlos and asks Root to continue until the phone reaches normal sessions
reliably. Root now runs as hostUID1000; do not stop the daemon hosting an active
turn. The current handoff is .agents/reports/exec022-codex-resume-handoff.md.
Saved-server app23952 passes edit/remove/duplicates/credentials/relaunch/token-change
checks and is installed on iPhone18 Pro; automatic launch was denied while locked.
The normal daemon still needs its listener. An optional private daemon.json startup
file and authenticated managed-restart checks are being completed under EP022.
Keep existing preview/catalogues unchanged and install only matching checked
packages. Arrange the one-time normal restart after this active turn finishes.
The owner now permits different source builds to attach to the same daemon when
their protocol matches. CARLOS_STRICT_DAEMON_BUILD=1 opts into the former exact
build guard; protocol and explicit native endpoint checks remain mandatory.
EP022 also fixes late replies from cancelled source/output watches, including
the owner's superseded-watch status and old-cursor disconnect reports. Keep
the current normal daemon alive until this turn and every other session are idle.
Checked runtimefa5890c2 is installed with backups at~/.local/lib/carlos/backups/20261117T220849Z.
Phone source122768f838c06ad4231a passes24 simulator tests and is installed and
launched on iPhone18 Pro. Idle restart and authenticated LAN-check user services
completed the restart; the independent check confirms authenticated access,
this session and Mac LAN reachability. The owner has resumed from the phone.
`.agents/reports/exec022-watch-phone-handoff.md` records checks, original failures,
unchanged successful rechecks and the remaining EP022 work.

The owner can test `target/preview-9cc99ff/try-carlos`, a fully checked snapshot that predates
only the narrow final API/compiler corrections; it keeps separate Carlos session state.
No installation, merge or push is included in delegated approval.

On2026-11-16 the owner also asked to test the current branch's CLI. The new frozen
`target/preview-87ceab5/try-carlos` copies the matching CLI/daemon/helper package from
the complete EP021b debug/native gate02, artifacts native-history-gate-AeiLPS. It includes
checked Codex paging and complete fileChange/MCP sources, before unfinished Claude Core
paging. Its launcher uses separate Carlos state at~/.local/state/carlos-spike-87ceab5.
preview-source.json records all three hashes and the checked runtime commit87ceab5.
The original two previews and the installed Carlos remain unchanged. Do not overwrite
this new owner preview during further builds either.

On2026-11-16 the owner reported flashing footer labels, an unhelpful Requesting label and permanent copy confirmations. Checked runtime2fbe4c4 fixes those issues. The new immutable target/preview-2fbe4c4/try-carlos packages a matching CLI/daemon/helper after both full Rust configurations/lints,31 surrounding terminal workflows, copy/source/notice checks and all four exact frozen-preview regressions pass. It includes the checked Claude native-window consumer, with separate state at~/.local/state/carlos-spike-2fbe4c4. preview-source.json records hashes and the narrower preview-check scope; the final EP021b release/native/performance gate remains pending. Do not overwrite any existing owner preview.

The owner later reproduced a second footer flash during log appends: the terminal briefly replaced real status with a visible-source loading hint. Runtime2a0a0c4 removes that replacement; checked native prompt recall follows in1347891. The new immutable target/preview-1347891/try-carlos includes both fixes and the cross-catalogue Claude resume advice. Both Rust configurations/lints, native/history/prompt daemon checks, exact source/terminal checks, matching-package smoke and all5 frozen-preview regressions pass (footer-source-flash-preview-01.log,5.730s). It uses separate state at~/.local/state/carlos-spike-1347891; resume a session from another preview with its native backend address, since Carlos logical IDs belong to their original catalogue. preview-source.json records matching binary hashes and check scope. The complete EP release/native/performance gate remains pending. Do not replace this or any earlier owner preview.

The owner requested a mid-EP whole-roadmap size review on2026-11-16. Fable5.2 medium
found avoidable panel-driver and command-wrapper repetition while supporting the shared
owner/client design. Its full report and checked decisions are
`.agents/reports/exec020-fable-bloat-review.md` and `exec020-bloat-review-disposition.md`.
EP021a completed the shared panel read settlement/paging in carlos-client and removed the
matching terminal controllers before any Swift/web consumer uses those families. The final EP020 review also
requires one exhaustive daemon command-admission class in EP021a before EP021b adds commands,
so a new mutation cannot silently use the read allowance. Preserve complete supported
lists and stale-reader checks. Use named payloads for new operator commands; examine
the existing wrappers alongside that shared-read cleanup. WebSocket access does not
freeze the provisional protocol. This advisory review does not replace EP020's final reviews.

ExecPlans 019a through 022 are listed in `docs/engine-architecture.md`. Only the next plan
is written in full. On 2026-11-14, the owner delegated approval of completed EPs to Codex and asked it to continue through the next EPs while the owner sleeps. Before each auto-approval, also obtain a Fable 5.2 medium review of the code and whole roadmap, as the owner requested in the same conversation. Use judgment to address or explain each finding, then record the auto-approval and continue after all checks and the direct engineering review pass. Do not stop at the former per-EP human gate. On 2026-11-14, the owner further directed: no more reviewers until the end of EP019b. Continue implementation and tests without new slice or follow-up reviewer sessions; obtain the final engineering and Fable reviews when the whole EP is ready. This instruction supersedes the repository's earlier per-slice review cadence, not its correctness checks. This grant does not authorize merging, pushing or replacing the installed spike binary; keep those actions separate.

The final EP019b Fable review found that live owners remain allocated until daemon shutdown and closed non-Codex sessions need an explicit recovery command. EP020 must first add a typed session stop/reopen path, with actual multi-client/native tests, before enabling its network listener. Stopping a session releases its owner and native processes while keeping the saved conversation. This is a required first milestone, not optional later cleanup. The review remains `.agents/reports/exec019b-fable-final-review.md`; its final EP019b dispositions belong in the final report.

ExecPlans 009 to 015 remain under `.agents/execplans/`. Their status was not reviewed when the
daemon split started. They remain outside the iPhone spike; its next step is EP022.


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

- [x] `.agents/done/EXECPLAN_020_network_access.md` (completed and approved under delegated authority 2026-11-16; corrected full gate, matched measurements and both final Fable passes recorded)

- [x] `.agents/done/EXECPLAN_021a_shared_panel_reads.md` (completed and approved under delegated authority2026-11-16; corrected full gate, exact-pair measurements and both end reviews recorded)

On2026-11-16 the owner explicitly requested pushing the current branch upstream. Codex first pushed iphone-spike atbb9be01 and later pushed through46c6367, with origin/iphone-spike tracking. No merge or force push occurred. The later local evidence/review work continues under the same spike plan.

- [x] `.agents/done/EXECPLAN_021b_native_phone_data.md` (approved under delegated authority
  on2026-11-16 at20:33:35 UTC; corrected full gate06, fresh exact-pair evidence and final
  engineering/Fable decisions recorded; archived plan unchanged)
