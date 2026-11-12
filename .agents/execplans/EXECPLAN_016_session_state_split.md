# Give entries stable ids and separate session facts from terminal state


This ExecPlan is a living document. Keep `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` current as work proceeds. Maintain this document according to `.agents/PLANS.md`. Follow `.agents/CODE_STYLE.md`: give each new Rust module one concern, keep files below 400 lines, keep functions below 60 lines, and put substantial tests in separate files.

## Purpose / Big Picture


Carlos users should see exactly the same transcript, controls, drafts, approvals, and backend requests after this work. Developers will gain a session state that they can exercise without constructing a terminal widget. Carlos will assign each transcript entry an id that stays attached to that entry as its text changes. Tests will prove that reloading the same history preserves ids when Carlos can identify the entries, and that rewinding never redirects a delayed update to an unrelated row.

A daemon is a process that keeps sessions running while clients connect and disconnect. This plan prepares the current single crate for that later process. It does not create a daemon. Plan 017 must be able to move `src/session/` and its terminal-free dependencies into `carlos-core` by moving files and adjusting module exports and Cargo dependencies. Plan 017 must not have to separate widgets from facts or rewrite session handlers.

This plan delivers three things: Carlos-owned entry ids, separate `SessionState` and `ViewState` structs, and Claude-to-common-JSON translation inside `src/claude_backend/`. Keep `BackendClient::take_events_rx() -> Result<mpsc::Receiver<String>>` and the current JSON messages. Do not add a crate or workspace, a backend-neutral typed event enum, numbered events, a socket, a session manager, or several sessions per process. Do not implement the phone screen, paged client history, new approval choices, output fetching, or new tool summaries. Those belong to later plans.

## Progress


- [x] (2026-11-11 UTC) Read the requested documents in order and verify the relevant survey claims against the working tree at `3f1916b2f7afd3b5e8291744923b329f2ef083af`.
- [x] (2026-11-11 UTC) Inventory all 29 `AppState` fields, including the field behind the `dictation` feature, and decide how to split their mixed children.
- [x] (2026-11-11 UTC) Attempt the default release build; verify that missing `libclang` and `cmake` prevent it in this container.
- [x] (2026-11-11 UTC) Build the unchanged source with `--release --no-default-features`; run the synthetic case three times and the repository history fixture three times. Record commands and results below.
- [x] (2026-11-11 UTC) Milestone 0: `cmake`, `clang` and Bun 1.3.10 installed in the development container. On Rust 1.100.0 with default features: release build passes, `cargo test` passes (705 passed, 1 ignored), `bash scripts/test.sh` passes (165 tests). The binding before-change baseline is the Rust 1.100.0 one under `Artifacts and Notes`.
- [x] (2026-11-11 UTC) Milestone 1: plain types live in `src/session/models.rs`, styled types in `src/app/view_models.rs`. 35 files, all in the allowed list. Gates: `cargo test` 705 passed, `scripts/test.sh` 165 workflows passed, review verdict PASS (see `Reviews`).
- [x] (2026-11-12 UTC) Milestone 2: the transcript store in `src/session/` owns entries and assigns `EntryId`; backend maps, prompt records, approval cards and label jobs hold ids, not positions. Two review rounds (FAIL with 5 MAJOR, then PASS on the delta). Gates: `cargo test` 822 passed, 171 terminal workflows, no speed loss.
- [x] (2026-11-12 UTC) Milestone 3: the Claude backend translates native events; the turn-end state machine runs when the app handles a line (`BackendClient::handle_event`, called in `src/app/input_dispatch.rs`). One squashed commit on `main` (`ac02f0a`, 91 files). Seven review rounds; final verdict PASS. Gates: `cargo test` 778 passed in 3.7 s, 168 terminal workflows, no speed loss.
- [x] (2026-11-12 UTC) Milestone 4: `AppState` holds `session: SessionState` (`src/session/`) and `view: ViewState` (`src/app/view_state.rs`); transcript, item, turn, Ralph-queue and approval logic moved to the session. Six review rounds (five by Codex, the last by Opus 5.5: PASS WITH ISSUES, seven MINOR). Gates: `cargo test` 855 passed in 3.7 s, 174 terminal workflows twice, no speed loss.
- [x] (2026-11-12 UTC) Milestone 4 follow-up A: MINOR findings 1, 2, 4, 5 and 6 of the final milestone 4 review cleaned up (`a2a5d31`, net 116 lines removed). Delta review by the same Opus reviewer: PASS WITH ISSUES, four small MINOR; the integrator applied two (`#[must_use]` on the six session methods that return retired ids; an explicit before/after id comparison in `AppState::set_pending_approval`), rejected one (`begin_approval_answer` keeps its `bool`: two tests assert that it refuses a card that is not active) and left one (`retire_approval` stays `pub(crate)` because an app test calls it). Still open from that review: finding 3 (for plan 017) and finding 7 (existing defect: a queued Claude card of the old session survives `reset_for_forked_thread`).
- [x] (2026-11-12 UTC) Milestone 4 follow-up B: the full live-event corpus (24,300 mutated streams, three backends) compared with the pre-plan commit `a59221c`: 23,099 exact, 867 differ only in a redraw log with identical effective ranges, 334 differ in session results, no panic. Six causes; two were regressions from milestone 2 and are fixed (a Claude guard that unbound Codex and Grok sources when both backend flags were set; the common history loader adopting rows of another thread without adopting the thread). Neither was reachable through ordinary backend output. Opus delta review: PASS WITH ISSUES (MINOR only). Details and the four differences that stay: `.agents/reports/exec016-preplan-comparison.md`.
- [x] (2026-11-12 UTC) Milestone 5, split into four parts so that each change stays under about 60 files:
  - [x] (2026-11-12 UTC) 5a runtime settings: the session owns capabilities, current and pending model / effort / summary and the model catalog (`src/session/runtime_settings.rs`, `runtime_settings_updates.rs`); the view owns the settings editor (`RuntimeSettingsView`). 37 paths. Opus review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 874 passed, 174 terminal workflows twice, 3,000 sampled streams equal to the previous commit.
  - [x] (2026-11-12 UTC) 5b input history: the session owns prompt records, entry-to-turn links and native rewind positions (`src/session/input_history*.rs`); the view owns navigation index, selected entry, restored drafts and rewind mode (`InputHistoryView`). The same round fixed MINOR findings 1, 2 and 4 of the 5a review and made the Codex undrained-read test independent of timing (0 failures in 400 runs, 100 of them under build load; 20 ms where it took 1.3 s). Five commits. Opus delta review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 882 passed five times with no intermittent failure, 174 terminal workflows twice, 3,000 sampled streams equal to the previous commit.
  - [x] (2026-11-12 UTC) 5c tool-label summaries and plain helpers: the session owns label bindings, the pending queue, the label worker, the accepted on/off preference and the workspace path (`src/session/summary_state.rs`, `summary_updates.rs`); the shell, tool, diff and source-path helpers moved verbatim into terminal-free session modules; the unsubmitted toggle stays in the view. 76 paths. Opus delta review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 890 passed ten times in a row, 174 terminal workflows (after the test repairs below), no speed loss.
  - [x] (2026-11-12 UTC) 5d rate limits: the session owns the quota windows, the pending quota read with its deadline and the next-read time (`src/session/usage.rs`, `rate_limits.rs`, `usage_updates.rs`); the view owns the next countdown redraw (`view.quota_next_draw`) and the colours. 26 paths. The same round fixed MINOR findings 1 to 3 of the 5c review. Opus delta review: PASS WITH ISSUES, two MINOR, both fixed by the integrator (`has_command` check before the app builds a label key; the hermetic startup test now requires `1 passed` from its child). Gates: `cargo test` 897 passed eight times in a row, 174 terminal workflows, no speed loss.
- [x] (2026-11-12 UTC) Milestone 6, split into four parts by the worker (`.agents/reports/exec016-m6-split.md`):
  - [x] (2026-11-12 UTC) 6.1 Claude metadata and optional reads: phase, capabilities, observed model, inherit flags, quota windows and the context read fields live in `src/session/claude/`; the settings-editor pair and the countdown redraw stay in the view; the Claude inherit and session-switch rules for runtime settings moved into the session (closes MINOR 3 of the 5a review). 44 paths. Fresh Opus review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 911 passed six times, 174 terminal workflows twice, 3,000 Claude streams equal to the previous commit, no speed loss.
  - [x] (2026-11-12 UTC) 6.2 native process work: task records, run memory, stop slots, process activity and browser request facts live in `src/session/claude/`; `ClaudeTasksView` (visibility, scroll, selection) and answer drafts stay in the view. The three bridge items of the 6.1 review are closed (the context revision lives in the session with one increment per invalidation; `observe_claude_model` invalidates context itself; `generation` is passed in for the compaction key). 48 paths. Opus delta review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 932 passed six times, 174 terminal workflows twice, 3,000 Claude streams equal to the previous commit, no speed loss.
  - [x] (2026-11-12 UTC) 6.3 accepted prompts and edited-branch requests: the accepted-prompt registry, the submitted edit text and the edit request progress live in `src/session/claude/{prompts,edit,prompt_transcript}.rs`; the composer, the unsent drafts behind F5, the failed badge and the queue-or-steer choice stay in the view (`src/app/claude_edit_view.rs`). 26 paths. Opus delta review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 952 passed six times, 174 terminal workflows (one run failed the known racy `test_codex_f2_phone_process_stop_failure...` workflow), 3,000 Claude streams equal to the previous commit, no speed loss.
  - [x] (2026-11-12 UTC) 6.4 conversation history and event fences: enabled, generation, epoch, notice keys, reset key, awaiting-init, history records and unknown-source prompt records live in `src/session/claude/{events,history,notices,notifications,reset}.rs`; named phase transitions replace the generic setter; `reset_claude_for_switch` is the one session-side reset for a switch; the dead `edited_prompt` view field is gone. `ClaudeUiState` now holds only `answers` (milestone 7). 66 paths. Opus delta review: PASS WITH ISSUES, MINOR only; the reviewer compared every fence line by line. Gates: `cargo test` 973 passed six times, 174 terminal workflows twice, 3,000 Claude streams equal to the previous commit, no speed loss.
- [ ] Milestone 7. Milestone 4 already did the approval half (`ApprovalState`, queues, card ids, the single write point, consent in the view). The worker split what remains into three parts (`.agents/reports/exec016-m7-split.md`):
  - [x] (2026-11-12 UTC) 7.1 plain question and form descriptions: the parsers `ask_fields`, `mcp_fields`, `schema_options` and Codex `question_fields` and `QuestionField` live in `src/session/questions/`; everything that builds an answer `Value` and every editor stays in the view. Same round: `safe_label` / `safe_text` moved to `src/session/text.rs`, `start_claude` narrowed, tests use the phase test setter (MINOR 1 to 3 of the 6.4 review). 26 paths. Fresh Opus review: PASS WITH ISSUES, MINOR only. Gates: `cargo test` 986 passed six times, 174 terminal workflows twice, 3,000 streams equal to the previous commit, no speed loss.
  - [ ] 7.2 Codex callback facts and submitted replies. Also: `NativeDraft.codex_blocking` is a view copy of the backend's `isBlocking`; derive it from the session's callback record or record why the copy may stay (MINOR 1 of the 7.1 review). First: move the Claude parser tests out of `src/tests/codex_question_tests.rs` and order the module declarations in `src/session/mod.rs` (MINOR 3).
  - [ ] 7.3 Claude callback commands and plain replies. Also: state whether the "unsupported request kind" rules in `NativeDraft::from_payload` move or are computed from session facts (MINOR 2).
- [ ] Milestone 8: separate Codex queue and goal facts from their editors.
- [ ] Milestone 9: separate Codex settings, task readers, and reconnect handlers from the view.
- [ ] Milestone 10: move the remaining session handlers and enforce the terminal boundary.
- [ ] Milestone 11: run the complete checks, compare speed, and record the engineering review.

- [x] (2026-11-12 UTC, handover) Recovered the Claude session and restored its named stash at base `91570ea`. The restored 37 source files exactly match `/tmp/carlos-m7b/source-audit.json`. The saved part 7.2 passed its build, release build, style checks, 990 Rust tests and 174 terminal workflows before the worker stopped. The resumed Rust run passes 990 tests in 3.76 s.
- [x] (2026-11-12 UTC) Milestone 7 part 7.2: direct handler review PASS, 990 Rust tests, 174 terminal workflows (78.211 s), release build, format and style pass. Seven alternating speed pairs against `a59221c`: synthetic replay 48.86 / 48.20 ms, layout 14.97 / 14.98, draw 3.88 / 3.65, scroll 0.70 / 0.70, typing 0.60 / 0.60. Small fixture replay 1.11 / 1.17 ms, draw 0.48 / 0.50. No repeatable slowdown. Report: `.agents/reports/exec016-m7b-report.md`.
- [x] (2026-11-12 UTC) Milestone 7 part 7.3: Claude callback commands, unsupported-kind checks, browser notices and replies now live in the session; local answers live in ViewState and ClaudeUiState is removed. 992 Rust tests, 174 terminal workflows, release build/check, format/style and direct review pass. All 3,000 streams (51,390 checkpoints) exactly equal the parent; pre-plan outputs equal part 7.2 byte for byte, with the same 103 classified H/T/F differences and no panic. Report: `.agents/reports/exec016-m7c-report.md`.
- [ ] Milestone 8:
  - [x] (2026-11-12 UTC) Part 8.1: native queue facts, reads, submitted writes and exact recovery reports. 993 Rust tests, 174 terminal workflows, release build, style and direct review pass. All 3,000 parent streams match exactly; pre-plan outputs equal part 7.3 byte for byte, including the same classified differences and no panic. Queue-specific behavior is covered by enumerated Rust/terminal tests; this inherited corpus contains no queue operation. Report: `.agents/reports/exec016-m8a-report.md`.
  - [ ] Part 8.2: native goal facts, reads, accepted changes and Ralph guards; keep goal editors and confirmations local.

## Surprises & Discoveries


- Observation: Claude already has a translator in `src/claude_backend/translate.rs`, but the app decides when and how to call it.
  Evidence: `src/app/claude_sdk_events.rs::main_items` suppresses translator-produced usage, exit-plan approval, and turn lifecycle lines. `ClaudeUiState::completions` supplies prompt completions only after a native result and an idle event. Moving only `main_items` would change turn completion and duplicate events.
- Observation: Claude history also calls the translator from app code.
  Evidence: `src/app/claude_session_history.rs::append_record` maintains a separate translator, calls `begin_history_snapshot`, and accepts only item messages. Its chunk reader bounds a record at 16 MiB. The backend must take over this path too.
- Observation: several apparently plain structs contain terminal widgets indirectly.
  Evidence: `CodexCallback::draft` reaches `NativeDraft.fields -> AnswerField.editor: TextArea`; `QueueEdit` and `GoalDraft` contain `TextArea` directly. `RateLimits` imports `weekly_usage`, which imports ratatui colors. `state_transcript` imports read-label helpers from `transcript_render`, which re-exports styled render code.
- Observation: backend code currently calls back into the app.
  Evidence: `src/codex_snapshot.rs` calls `crate::app::reconnect_history` and `crate::app::reconnect_panel`. The latter builds `AppState` in `src/app/codex_reconnect_snapshot.rs`. A terminal-word grep of only `state.rs` would miss this dependency.
- Observation: read grouping leaves an empty entry rather than removing an element.
  Evidence: `state_transcript.rs::coalesce_successive_read_summary_at` changes the preceding message and clears the current message. Both entries need ids, even though the renderer hides the empty one.
- Observation: the benchmark's memory figures cannot measure this change.
  Evidence: `src/app/perf_session.rs::current_rss_kib` uses `strip_prefix("VmRSS:")?` inside its loop and returns `None` at the first nonmatching line. Every run below prints zero. Do not treat those zeros as measurements or fix that unrelated bug during this plan.
- Observation: this container can measure transcript work with dictation disabled, but cannot build the default features.
  Evidence: plain `cargo build --release` fails to find `libclang`; `WHISPER_DONT_GENERATE_BINDINGS=1 cargo build --release` gets further but cannot start `cmake`. `cargo build --release --no-default-features` succeeds. No packages or binaries were installed.
- Observation: other work advanced the branch while the author wrote this plan, without changing the measured Rust source.
  Evidence: final HEAD was `e46daddb0f70e178935fdcb9d62fc98ba5fbbd62`. The diff from the baseline revision touched only `.gitignore`, `docs/engine-architecture.md`, `docs/ios-build.md`, and `scripts/ios-mac.sh`. The updated design adds a later shared client crate and a future Rust 1.100.0 pin; step 1 remains unchanged.

- Observation: `claude_backend::transport_tests::slow_ui_stops_the_helper_and_settles_unanswered_requests` failed once in seven full `cargo test` runs on the unchanged tree (assertion `backlog overflowed`), and passed in every isolated run on Rust 1.93.0 and 1.100.0.
  Evidence: runs of 2026-11-11 by the integrator. Treat one failure of this test as an existing timing fault: rerun the suite once before blaming a change. Milestone 3 touches this area and must not make it worse.

- Observation: Rust 1.100.0 prints 3 warnings on the unchanged tree; one suggests `try_update` in place of the current atomic update call.
  Evidence: release build of 2026-11-11. Not fixed in this plan.

- Observation: `DisplayMessage` borrows the stored `Message`, so milestone 2 can read the entry id through the display view without a copied id. The render cache already saves the row plan and no longer keeps compact message copies.
  Evidence: milestone 1 worker report, `src/app/view_models.rs`, `src/app/render_cache_state.rs`.

- Observation: speed figures taken while another build runs on the machine are useless: the same binary measured 84 to 105 ms replay under load and 49 ms a few minutes later. Timing tests also fail more often under load (one terminal workflow timeout, one 20 ms stdin deadline in `unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls`; both passed on retry).
  Evidence: integrator runs of 2026-11-11. Method from now on: keep a release binary of the pre-change commit (`/tmp/carlos-base`, commit `a59221c`) and alternate runs of the old and new binary in one loop; compare the medians of those runs, not a figure from another hour.

- Observation: milestone 1 side-by-side medians over 7 alternated runs, old then new: replay 50.39 / 49.30 ms, full layout 15.92 / 15.12, full draw 3.47 / 3.81, scroll p50 0.71 / 0.71, typing p50 0.60 / 0.60. Full draw varies between 3.4 and 4.5 ms for both binaries; no regression.
  Evidence: integrator runs of 2026-11-11 with load average 9 to 13.

- Observation: a synchronous test fixture that applies each native frame at once (`normalized_app`) cannot show defects that depend on which thread has seen which event. 729 Rust tests and 165 terminal workflows passed on a branch with three reproducible MAJOR ordering defects.
  Evidence: milestone 3 reviews. When code moves a decision across a thread boundary, require a differential test against the old logic over generated event orders, and real-thread tests with barriers.

- Observation: the parity audit of milestone 3 found more lost rules than four reviews did. The worker listed every guard, default, ignored error and ordering rule of `main`'s Claude handling with its new location, and compared 48,000 mutated native frame streams between `main` and the branch (identical app state, before and after EOF).
  Evidence: `.agents/reports/exec016-m3-parity.md` (parity table, error split, mutation method). Use the same two tools for every later milestone that moves handling code: milestones 6 to 10 move the remaining Claude and Codex handlers.

- Observation: `protocol::history_transport_tests::unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls` (Codex, 20 ms deadline) fails about once in ten full runs, on the unchanged tree and after every milestone. It is not caused by this plan.
  Evidence: integrator runs of 2026-11-11 and 2026-11-12. Unfinished work: replace its wall-clock deadline with a barrier. Until then, rerun once when only this test fails.

- Observation: the full differential test (1,221,882 schedules) made `cargo test` take 60 s; the pre-commit hook runs `cargo test`. The default test now runs 46,458 schedules in about 1.1 s and the full enumeration is `#[ignore]` (`cargo test -- --ignored`).
  Evidence: milestone 3 fix rounds. Keep `cargo test` under 8 s in every later milestone.

- Observation: final milestone 3 speed check on `main` (`ac02f0a`), 7 alternated runs, old then new medians: replay 46.73 / 46.49 ms, full layout 14.61 / 14.57, full draw 3.41 / 3.41, scroll p50 0.70 / 0.70, typing p50 0.59 / 0.60.
  Evidence: integrator runs of 2026-11-12.

- Observation: milestone 2 changes what the user sees in three Claude cases. All three follow from the binding identity rules and correct duplicates or overwrites of the previous code. (1) On resume, the SDK can send again an assistant message tied to in-flight tool calls; the previous code added a second assistant-text row, the store now reuses the original row. This is reachable with real data. (2) A tool id that prints like a Carlos cursor id no longer overwrites the reasoning or text row; no real capture shows such an id. (3) A repeated prompt UUID in history yields one prompt row and one rewind entry, not two; the helper already removes duplicate UUIDs, so this needs manufactured input.
  Evidence: 28,218 mutated histories (10,122 Codex, 7,000 Claude, 11,096 Grok): 25,915 match the previous commit exactly, 2,303 Claude histories differ only in these declared classes; reviewer's replay probes. Details: `.agents/reports/exec016-m2-fixes.md`.

- Observation: the mutated-history comparison found two defects that neither the tests nor the first review found (tool facts attached by a row's final role; prompts recorded for keys that belonged to other rows).
  Evidence: `.agents/reports/exec016-m2-fixes.md`. Third time this method pays; it is mandatory for milestones 4 to 10.

- Observation: milestone 2 speed check, 7 alternated runs, old then new medians: replay 46.04 / 45.76 ms, full layout 14.36 / 14.25, full draw 3.37 / 3.40, scroll p50 0.70 / 0.70, typing p50 0.59 / 0.60, append p50 0.60 / 0.60.
  Evidence: integrator runs of 2026-11-12 on a quiet machine.

- Observation: milestone 2 (`d98a64d`) introduced a panic: `item["id"]` on a JSON map in `notification_tools.rs` panicked for a completed tool item without an id. No real backend sends such an item, but one would have crashed carlos, and in the daemon every session. Milestone 4's live-event mutation comparison found it; milestone 2's comparison covered history loading only. Fixed in `2cbfedc` with the pre-plan fallback behaviour restored.
  Evidence: `.agents/reports/exec016-idless-panic.md`. Rule from now on: every milestone's mutation comparison covers live events and history for all three backends, uses BOTH the previous commit and the pre-plan commit `a59221c` as references, and counts any panic as a failure even when the reference panics too.

- Observation: rounds are slow because of Codex run time and the number of rounds, not because of the gates. Measured: the integrator's gates take about 5 minutes; a fix worker 13 to 57 minutes; a Codex review 9 to 29 minutes, and delta reviews were no shorter than full ones because each delta brief added new attack lists. Milestone 4 touched 164 files and took six rounds.
  Evidence: log timestamps of 2026-11-11 and 2026-11-12. Changes: gates and review run at the same time; a delta brief has two tasks (check the fixes, check the changed lines); the integrator fixes MINOR findings without a separate round; a fix worker reruns only the proofs its change can affect; milestones 5 to 10 are split when they reach about 60 files; a requirement stricter than the previous code goes to the plan that needs it.

- Observation: milestone 4 final speed check, 7 alternated runs, old then new medians: replay 48.10 / 47.08 ms, full layout 14.93 / 14.83, full draw 3.58 / 3.40, scroll p50 0.70 / 0.70, typing p50 0.60 / 0.59, working p50 0.60 / 0.60, append p50 0.60 / 0.60.
  Evidence: integrator runs of 2026-11-12.

- Observation: the Codex test `unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls` fails under load because its 64 calls use a 20 ms deadline: when the stdin writer has not confirmed the write within 20 ms, the call returns "Codex stdin deadline expired; request may already have been accepted" and the test expects "Timeout waiting". It also takes about 1.3 s of the 3.7 s suite. It failed once inside `scripts/test.sh` on 2026-11-12 with a reviewer running at the same time. Fixed on `daemon-split` on 2026-11-12 (peer receipts in place of the deadline); still present on `main`.
  Evidence: `src/tests/protocol_history_transport_tests.rs:100-111`, `src/protocol_writer.rs`.

- Observation (5a, behaviour kept on purpose; candidates for the daemon plan): the apply method overwrites all current runtime fields when any pending field exists; the set and queue methods accept effort and summary even when the backend's capabilities disable them; Codex keeps raw strings from native acknowledgements and launch overrides while ordinary setters trim.
  Evidence: `.agents/reports/exec016-m5a-report.md`.

- Observation: many tests depended on wall-clock timing, on the pre-plan commit as much as now. Repaired on `daemon-split` on 2026-11-12: three Rust tests (`unanswered_large_read_ids_...`, `a_native_event_flood_...`, `targeted_stop_timeout_...`) and three terminal workflows (the Codex goal editor workflow sent a resize before carlos had handled a paste, reopened the editor before a rejection arrived, and sent Escape then `p` so close together that the terminal read Alt+P; the Claude rejected-interrupt workflow required one order of result and interrupt reply although the TypeScript controller can forward either; the terminal emulator used by the tests crashed on an erased half of a wide character). No production code changed. Measured before the repair: goal editor 2 failures in 150 quiet runs on the pre-plan commit; the plan did not raise any rate. After the repair: 300 quiet and 100 loaded runs of each workflow, 20 full terminal suites and 20 `cargo test` runs without a failure. Still timing-dependent under heavy build load (not repaired): `late_large_history_replies_drain_without_closing_healthy_native_work`, `a_slow_read_does_not_kill_native_work_or_leak_its_late_reply`, `cached_and_uncached_history_do_not_wait_for_model_or_trigger_calls`.
  Evidence: `.agents/reports/exec016-terminal-test-races.md`, `.agents/reports/exec016-m5c-report.md`.

- Observation: the Claude TypeScript controller forwards SDK events and control replies on two asynchronous paths, so carlos can receive "interrupt rejected" before a result that the native CLI wrote first. Carlos handles both orders correctly (the answer completes, no stop marker).
  Evidence: `.agents/reports/exec016-terminal-test-races.md`, section "Claude: native order differs from controller order".

- Observation: a 3,000-stream sample compared with the pre-plan commit showed 278 differing streams where the known causes predict about 120. All 278 were classified event by event: 158 come from the comparison harness itself (after `a2a5d31` removed the test-only last-writer reader, the harness read `view.status_hint` in its place and so kept an old hint for ever; production draws neither string), 102 are H, 12 F, 4 S, 2 T. Without the harness artefact 121 differ. No unintended screen change and no unintended backend call. A harness that compares the status string must read `session.status()` only.
  Evidence: `.agents/reports/exec016-m5d-report.md`, section P4; `/tmp/carlos-m5d/p4-ledger.json`.

- Observation: a fourth terminal workflow failed once under load, `test_codex_f2_phone_process_stop_failure_preserves_draft_and_exact_selection` (`tests/ui/test_codex_panel.py:54`, timed out waiting for the main draft after the process panel). Same class as the three repaired workflows: the test sends the next key before carlos shows the effect of the previous one. Unfinished work: audit all terminal workflows for this pattern.
  Evidence: integrator's gate run of 2026-11-12.

## Decision Log


- Decision: put session types and handlers under `src/session/`, outside `src/app/`, while keeping the single binary crate.
  Rationale: a directory that cannot import the app is a concrete set of files that plan 017 can move. Merely nesting a facts struct inside `AppState` would leave its methods tied to widgets.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: give every stored `Message` an opaque `EntryId` with a Carlos-generated namespace and an increasing counter. Use the existing `uuid` dependency once per namespace, not once per appended row.
  Rationale: a backend id identifies backend work; a vector index identifies a current storage position. Neither identifies a Carlos transcript entry. A namespace plus counter prevents a temporary history builder or a reset from reusing a retired id and keeps appends inexpensive.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: preserve ids across same-thread replacement only through an exact source key, or through an explicitly retained in-memory prefix. Give all entries fresh ids when adopting a different backend thread or a cleared conversation.
  Rationale: matching repeated prose would attach state to the wrong entry. A fork is a new conversation even if it contains copied text. Carlos does not need to persist ids across process restarts in 016.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: split `input_history`, rather than assigning the whole current struct to the view.
  Rationale: navigation index, restored drafts, and rewind focus belong to the view. Submitted prompt records, entry-to-turn links, and `NativeUserHistory` positions determine which backend message Carlos forks before, so the session must own them.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: keep the current `status` string in the session, and move transient editor errors and local hints to the view where they originate. Let the status renderer choose the same visible text and precedence as today.
  Rationale: turn failure and backend status remain true without a terminal; an escape-key hint or field error belongs to one editor. Separate setters must prevent a local hint from overwriting a backend fact.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: move `summaries.enabled`, `cwd`, bindings, pending jobs, service, and replay flag to the session; move `summaries.draft` to the settings view.
  Rationale: Carlos computes tool labels independently of terminal width and receives them asynchronously. The settings editor's unsubmitted checkbox belongs to the device. Load the saved preference in app startup and pass its value into the session constructor, rather than importing a UI preferences module into the session.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: move `command_render_overrides` to the session despite its current name, and keep `tool_output` in the view.
  Rationale: an override holds a parsed command label that any client needs. `ToolOutputState` chooses folded output from terminal size and a local preference. Extract plain diff and source-path helpers from its renderer dependencies.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: split `runtime`, `claude`, and `codex` at their child fields. Accepted or pending backend requests belong to the session even if they contain recoverable user text. Unsent editor contents belong to the view.
  Rationale: otherwise callback drafts, queue editors, goal editors, and settings overlays pull terminal crates into the future core. Keep one owner for each value; identify submitted work by its request token, prompt UUID, or entry id rather than duplicating mutable state.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: move Claude translator cursors and the native result/idle completion tracker into `ClaudeClient`'s backend-side event normalizer. Keep the scheduler's accepted-prompt facts in the session.
  Rationale: the backend must emit complete common transcript and lifecycle messages before callers receive them. The session still decides Ralph continuations and local queues. The two sides must not independently emit turn completions.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: retain the current ordered-vector render caches, but make their positions private to the view and carry an entry id with every persistent entry reference.
  Rationale: rebuilding a hash lookup during every draw would threaten responsiveness. A view can resolve an id to a current offset once, then use the existing fast contiguous cache. Reset all position-based caches after replacement, just as Carlos does now.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: use a small synchronous change report for view invalidation and existing backend actions, without introducing a typed backend event model.
  Rationale: a caller needs to know which rows to redraw and which drafts to restore. It does not need a new event bus. Plan 018 will define numbered, typed events later.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: keep the no-dictation baseline as a comparable transcript baseline and require a default-feature preflight before accepting implementation milestones.
  Rationale: disabling dictation does not change the measured transcript operations, but it cannot prove that default-feature builds or terminal workflows pass. Missing tools are an environment gap, not permission to weaken the required checks.
  Date/Author: 2026-11-11 / Codex, plan author.
- Decision: record the newer design decisions without expanding 016 to add the shared client crate or pin a new compiler.
  Rationale: the shared client belongs to the later client/daemon work. The agreed compiler target is Rust 1.100.0 when available, while these reports used installed Rust 1.93.0. A worker who changes compiler must collect a fresh before-change baseline with that compiler, as the design now requires.
  Date/Author: 2026-11-11 / Codex, plan author.

- Decision: continue in this checkout on `daemon-split`, with the primary agent doing all code and review work directly. Do not start any worker or reviewer agent.
  Rationale: the owner explicitly requested this on handover. This overrides the earlier worker/reviewer transport instructions; the existing reviewer prompt still supplies the checklist. Preserve the named stash as a recovery copy. Do not install, merge or push this branch.
  Date/Author: 2026-11-12 / Codex, at the owner's request.

- Decision: borrow the active approval facts when sending its plain answer and split the browser/card borrows inside the session.
  Rationale: an existing allocation test caught a second full-payload copy in the first attempt. The corrected path preserves the original one-copy cost and all consent checks.
  Date/Author: 2026-11-12 / Codex.

- Decision: split milestone 8 into native queues and native goals. Move the shared Codex request counter with the queue controller; the remaining connection scope stays in its existing owner until milestone 9 and is passed as plain values.
  Rationale: both controllers use the counter. Moving it once preserves correlation across controllers without copying mutable facts or widening the queue change into the whole connection state.
  Date/Author: 2026-11-12 / Codex.

## Reviews

- Milestone 1, engineering reviewer (`.agents/reviewers/engineering_reviewer.md`, Codex `gpt-6.1-sol`, fresh session, 2026-11-11): verdict PASS. No BLOCKER, MAJOR or MINOR finding. Confirmed: no terminal dependency or row count in session models; `app/models.rs` re-exports plain types only; compact and full labels, diff styles, row counts and selection rows preserved; no per-frame message clone; no test weakened. The reviewer ran `cargo fmt --check` and `git diff --check`; it relied on the integrator for tests and speed.

- Milestone 3, engineering reviewer, three fresh sessions on branch `exec016-m3` (2026-11-11): FAIL, FAIL, FAIL.
  Review 1 (commit `a3865a3`): rejected interrupt can mark a good turn interrupted; stderr can complete a turn inside a stdout batch; UUID source keys differ between live and history; two MINOR (final sender waits forever after receiver drop; queue send under a lock at EOF).
  Review 2 (after fix `ab1d050`): four of five fixed; rejected interrupt still wrong for result-before-rejection; a stderr failure that fills the queue no longer stops the helper.
  Review 3 (after fix `f9e6a40`): a reset or a second prompt erases or blocks a held completion; stderr bypasses the overflow stop while a completion is held; the interrupt captures the reader's prompt state, not the app's; failed interrupts after EOF retain tickets. The reviewer also showed that the exhaustive tests modelled one prompt and one interrupt only.
  Outcome: redesign per the Decision Log entry on the Claude turn-end state machine. Full review texts: `/tmp/carlos-survey/m3-review*.md` (not kept in the repository).

  Review 4 (after the redesign `8cd709e`): event-order defects confirmed gone. New: a repeated reset key overwrote a transcript row; a pending call waited behind terminal delivery after helper death; a malformed SDK record that `main` ignored stopped the session; two MINOR.
  Review 5 (after `7c03e0f`): one MAJOR (stdout EOF set the `stopped` flag, so a later overflow no longer stopped a live helper), one MINOR (two test functions over 60 lines).
  Review 6 (after `35ebe60`): one MAJOR (client drop joined pipe workers and could wait without limit; caused by an integrator brief that wrongly required "no thread outlives the client"), one MINOR (test fixture and descriptor 100).
  Review 7 (delta review of `35ebe60..451be41`, saved reviewer session `01a1cd74-5188-7fd3-98fa-142c2da35f18`): PASS, no finding. Noted limit that `main` shares: each session replacement leaves blocked reader threads and pipe descriptors while a grandchild of the helper holds a pipe; they end when the holder exits.

- Milestone 2, engineering reviewer, saved session `01a1cdba-d79e-7410-b8c0-03cef56ff2ad` (2026-11-12).
  Review 1: FAIL. MAJOR: history reload skipped invisible native user messages (wrong prompt position and kind); two Claude sources with the same printed id shared a row; the history parser merged repeated keys before the store applied the ambiguous-key rule; a native command override hid a tool label (visible change); complete-history read grouping quadratic (also on the previous commit). MINOR: superseded Claude cursor aliases unbounded; the differential reference was not the unchanged old algorithm.
  Review 2 (delta, same session): PASS, no finding. Read grouping measured linear (2,057 steps for 1,024 reads, 8,201 for 4,096). All 14 unmutated fixture histories match the previous commit exactly.

- Milestone 4 (2026-11-12). Codex reviewer, saved session `01a1ce3e-c9f6-7971-939c-80a4214cdead`, five rounds, all FAIL:
  1: local hints overwrote session status; the approval confirmation hint was stored in session facts.
  2: stale approval consent survived a Claude session switch (security-relevant; never reached a commit); orphaned hint records; keyboard guidance stored in request details.
  3: a setter accepted a stale card id with changed facts; identical re-adoption lost the guidance; a parity test compared with `zip` and no length check.
  4: a second setter restored a retired card with its consent; a `Deref` on the card.
  5: no consent defect left; each Codex reply copied the request facts twice (MINOR, speed rule).
  The integrator's gate run then found one racy terminal workflow added in these rounds (7 failures in 50 runs; fixed by waiting for the completed screen).
  Final review by a fresh reviewer on Opus 5.5, medium effort: PASS WITH ISSUES, no BLOCKER, no MAJOR. It read the approval module and its callers in full and judged the design sound; it noted that the new code is stricter than the pre-plan code (which skipped confirmation for a non-string request id). MINOR findings:
  (1) key names in two session status strings (`src/app/codex_callbacks.rs:184`, `src/app/codex_lifecycle.rs:46`);
  (2) leftovers of the fix rounds: `PendingApprovalRequest.entry_id` is an `Option` that is always `Some`; `set_pending_approval` repeats the `same_request` check; `begin_approval_answer` always returns true outside the Codex form path; `TurnBoundary.first` and `observe_turn_append` are read only by tests (the boundary is still the saved index); `SessionUpdate.dirty_tail` is set only by a test helper; duplicated retire code in `codex_lifecycle.rs` and `codex_reconnect.rs`; double queue scans in `codex_callbacks::retire` and `claude_callbacks::cancel_callback`; nine identical doc comments in `transcript_commands.rs`;
  (3) for plan 017: app tests call `#[cfg(test)]` session methods, which will not compile across a crate boundary (needs a `test-support` feature); the session uses `crate::backend::{BackendKind, PromptOrigin}`; accessors return `&String` / `&Option<String>`;
  (4) `ViewState.status_hint` is read only by a test helper and `SessionState.status_revision` exists only for it;
  (5) weak tests in `turn_boundary_tests.rs` (an assertion that holds for any start value; a loop counter asserted; expected values that restate the implementation); 0.93 MB of JSON fixtures with no recorded way to regenerate them;
  (6) the view mirrors card lifetimes by hand at about eight sites;
  (7) existing defect, same on the pre-plan code: `reset_for_forked_thread` calls `clear_pending_approval`, which promotes the old session's first queued Claude card before the queue is cleared, so that card survives the switch (consent is reset, so no consent hole).
  The reviewer did not build or run tests and did not audit the moved Ralph, transcript, item-source, history-identity, summary and input-history code.

- Milestone 5 part 5a, fresh Opus 5.5 reviewer (medium effort, 2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed all 9 session facts and 9 view fields on the correct side (pending settings are session intents applied at the next turn; unsent editor values and overlay visibility are view state), unchanged order and guards in the split methods, unchanged defaults file, unchanged startup values for the three backends. MINOR: (1) the session constructor stores saved preferences as current model / effort / summary although no backend confirmed them (every backend overwrites them at start); (2) the session imports `ModelInfo` from `protocol_params`, which depends on the backend client and would hinder the crate split; (3) `record_runtime_choice` / `clear_pending_runtime_choice` let the app set any single field, and the app still decides the Claude inherit and session-switch rules: accepted until milestone 6 moves those rules; (4) a duplicated `normalize_non_empty`, a view method named `apply_model_settings` that only builds choices, a stale `allow(dead_code)` in `state_settings.rs`; (5) nine files outside the allowed list with field-path edits only (`claude_context`, `claude_models`, `claude_session_history`, `codex_reconnect_install`, `codex_settings`, `codex_settings_draft`, `input_events_keys`, `mouse_events`, `render_sections`).

- Milestone 5 part 5b and the 5a fixes, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: startup values unchanged for the three backends; the rewritten timing test still proves the cap, the ordinary-control check, late drain and recovery, and tests the client, not its helper; history fields on the correct side; rewind and fork requests unchanged for the three backends. MINOR: (1) in `src/tests/runtime_tests/startup_preferences.rs` the case for a new Codex thread passes saved values in as if Codex had reported them, which copies the override in `configure_codex_app` instead of testing it; (2) `record_prompt_history` and `discard_pending_prompt` return "the client must reset its cursor", a view concern in a session contract: with several clients this becomes a change notice (plan 018); (3) the session imports plain types from `crate::backend` (`RewindUserMessage`, `BackendKind`, `PromptOrigin`), which must move with it at the crate split (plan 017).

- Milestone 5 part 5c, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: the 12 deleted app files equal their session copies apart from visibility, imports and doc comments; ownership correct; the late-label rule holds for replaced, rewound and re-keyed entries; `src/summaries/` untouched; the two repaired protocol tests still prove their facts and the race was in the tests (the client wakes callers before it publishes its closure notice). MINOR: (1) `src/app/state_summaries.rs:23-32` builds the cache-entry string for every item start and completion, where the previous code built it only for a new label request (extra work on every history load); (2) the new startup test reads `CARLOS_REASONING_SUMMARY` from the environment; (3) the rewritten stop-timeout test no longer exercises the deadline thread, which `protocol_transport_tests.rs` (`timeoutMs: 40`) still covers; (4) for plan 017: the session owns `SummaryService`, so `src/summaries/` moves with it.

- Milestone 5 part 5d, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: quota arithmetic and pace thresholds moved verbatim; same colours and text; the split of `next_draw` loses no wakeup and adds none (walked through a future reset, an exhausted quota, a reset in the past, a reset exactly now, no quota, a read in flight, a failed read); the session usage code needs only `std` time and `serde_json`. MINOR, both fixed: the app built an item key before the command check; the hermetic startup test checked only its child's exit status.

- Milestone 6 part 6.1, fresh Opus 5.5 reviewer (medium effort, 2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: the four-part split is sound and assigns every `ClaudeUiState` field once; ownership correct on both sides; every handler compared with the previous commit (context receive keeps token check, slot release, revision check in that order, so an old read cannot restore counts after a compaction; quota requests at the same moments); `src/claude_backend/` untouched. MINOR: (1) generic `set_claude_phase`; (2) `tick_claude_context` writes an app-owned revision through `&mut u64`; (3) `observe_claude_model` relies on the app invalidating context first; (4) module order in `src/session/claude/mod.rs` (fixed). Split gap: the compaction key in `claude_notices.rs` uses `generation`, which stays in the app until part 6.4.

- Milestone 6 part 6.2, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: the three 6.1 items closed with identical behaviour; the moved task code equals the previous commit apart from visibility and derives; task events still run before the child-message filter; the bounded-failure order is unchanged; `/clear` keeps tasks, stops and browser facts, and a session switch resets them at the same point; the "working" checks changed only their access path. MINOR: (1) a session module loaded a test from `src/tests/` through `#[path]` (fixed: the file moved to `src/session/claude/stop_parity_tests.rs`); (2) `reset_claude_metadata` is misnamed now; (3) the app passes `enabled` to the context reader until part 6.4; (4) more `#[cfg(test)]` session helpers called from app tests (plan 017).

- Milestone 6 part 6.3, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed: no unsent text in the session (an edit enters it only after the fork call succeeded); no submitted request kept only in the view; the former `Option<EditState>` with a `Failed` variant is now a session request plus a view flag that never hold at once; registry bounds, rejection, retirement, `/clear` retention, session switch, native reset and `fail_session` equal the previous commit; `turn_submit.rs` and the backend normalizer untouched. MINOR: (1) `ClaudeEditView.edited_prompt` is never set in production and its fallback in `take_edit_prompt` is dead (five tests still seed it); (2) `reset_claude_metadata` misnamed; (3) `consume` builds an item key before it knows the prompt exists (no action).

- Milestone 6 part 6.4, same Opus reviewer, delta review (2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed fence by fence: generation `!=` current dropped; epoch `<` current dropped except the process set (callback, callbackCancelled, task, browserComplete, controlReply for start / capabilities / task.stop / usage.optional / context) and messages carrying `claudeContext`; missing or non-numeric values skip the checks; after close the same eight methods pass; `sessionChanged` needs a numeric generation `>` current and a string id; a repeated reset key returns before any write; init only while awaiting a reset; each named phase transition matches one place and condition of the old code. One timing difference judged unobservable: on a session switch, enabled / generation / Starting are set a few calls earlier, before the approval queue clear, which reads no Claude field. MINOR: (1) the session writes transcript text through `safe_label` and `safe_text`, passed in as `fn` pointers from `app::claude_question_state`; (2) `start_claude` is `pub(crate)` but used only by the session and tests; (3) several tests reach Ready through `ready_claude_after_edit_failure()`.

- Milestone 7 part 7.1, fresh Opus 5.5 reviewer (medium effort, 2026-11-12): PASS WITH ISSUES, no BLOCKER, no MAJOR. Confirmed with a script that the moved parsers equal the previous commit (two removed lines are restored by the view wrappers), that `safe_text` / `safe_label` are byte-identical, that everything building an answer `Value` stayed in the view with the same error strings, and that no editor state sits in the session. MINOR: (1) `NativeDraft.codex_blocking` view copy; (2) unsupported-kind rules decided in the view; (3) Claude parser tests attached to a Codex test file, module order.

- Milestone 7.2, direct primary-agent review at the owner’s request: PASS. Checked guards, typed identity bounds, retirement, candidate/revision/card checks, failed writes and exact reply conversion against `91570ea`. No BLOCKER or MAJOR. This was a self-review; no independent reviewer was invoked.

- Milestone 7.3, direct primary-agent review: PASS, no BLOCKER or MAJOR. All moved guards and fallbacks are listed in the report. The unchanged consent and payload-copy tests pass. The owner’s no-subagent instruction still applies.

## Outcomes & Retrospective


Milestones 0 through 7 are complete. The daemon split remains in its separate checkout; the main checkout has unrelated owner changes. The primary agent recovered part 7.2 from the named stash and completed the Claude callback split. Milestones 8 through 11 remain. No binary has been installed. Milestone 7’s seven alternating speed pairs gave synthetic replay 48.79 / 47.61 ms, layout 15.04 / 14.45, scroll 0.70 / 0.71 and typing 0.60 / 0.60; the small fixture replay was 1.09 / 1.06 ms. No repeatable slowdown. The original baseline notes below remain historical evidence; milestone 0 supplied the missing tools and the Rust 1.100.0 default-feature baseline.

## Context and Orientation


Run commands from `/var/home/wegel/work/perso/carlos`. `Cargo.toml` defines one binary package. `src/main.rs` declares the top-level modules; `src/app/mod.rs` declares app modules and connects `src/tests.rs`, which re-exports many app internals for tests. A worker must update that test bridge when moving a type. Do not preserve a terminal dependency merely to avoid updating a test helper.

`src/backend.rs::BackendClient` exposes controls and receives JSON strings through a channel. Codex owns its transport in `src/protocol*.rs` and connection/snapshot code in `src/codex*.rs`. Grok converts native messages in `src/grok_backend/translate.rs`. Claude currently sends `claude/controller` messages from `src/claude_backend/transport_reader.rs::envelope`; app handlers unpack the helper frame and translate its SDK messages. Here, an SDK message is a message produced by Claude's software library through the compiled controller under `helpers/claude-controller/`.

`src/app/state.rs::AppState` contains 29 fields. The complete assignment below covers each field and the children that cannot move intact. `Message` in `src/app/models.rs` currently contains `tool_output`, `role`, `text`, `kind`, and `file_path`. Its module also defines `StyledSegment`, `RenderedLine`, and `TerminalSize`. `Role` has User, Assistant, Commentary, Reasoning, ToolCall, ToolOutput, and System variants. `MessageKind` has Plain, UserSteer, Diff, and the renderer-produced ToolSummary. Keep the stored source text and the current distinction between commentary and final answers. Do not replace these types with the future phone protocol's entry enum.

`src/app/state_items.rs` constructs a Codex key from turn id, item id, and display part, then maps that key to a vector index. `with_item_part` handles a tool that produces several file diffs. `state_transcript.rs::append_message`, `append_diff_message`, `update_message_at_index`, `upsert_turn_diff`, and the streamed-item helpers mutate the transcript and invalidate render caches in the same methods. `state_summaries.rs` also changes tool labels after an asynchronous job finishes. These paths must converge on the entry store.

`src/app/item_history.rs::replace_history_from_thread` constructs a temporary `AppState`, fills it, then calls `state_thread.rs::replace_transcript_from`. Codex reconnect calls this through `codex_reconnect_install.rs::install_history_runtime`. `turn_submit.rs::apply_forked_thread_history` clears the old conversation before loading a new one. Grok instead clones a prefix in `snapshot_grok_rewind_prefix` and pushes it back in `apply_grok_rewound_history`. Claude reads history in `claude_session_history.rs`, changes sessions in `switch_session`, and clears a conversation in `claude_reset.rs`. All these paths must follow the same id rules.

The phone notes in `tmp/main-screen.md` sections 6 and 11 explain why a client needs entry identity, Markdown source, reasoning/tool completion, tool labels and failures, approval facts, context counts, and quota facts. They do not authorize implementing those future screens here. In 016, the session keeps existing facts and exposes them without colors or widgets. The view still wraps text, chooses one-row reasoning, colors output, folds tools, and draws approvals. Preserve current strings, output bytes, order, and choices, including current backend limitations.

- Decision: the Claude reader thread only translates. The turn-end state machine lives in `src/claude_backend/` but runs on the thread that handles events, at the moment the app handles a line, through one new `BackendClient` method. The interrupt control reads and updates the same state on that thread. This replaces the earlier decision in this log to run the completion tracker in the backend's reader-side normalizer.
  Rationale: on `main`, one thread decides when a Claude turn ends, in the order in which the app handles events. Running that decision in the reader thread created two clocks (read by the reader, handled by the app). Three independent reviews of milestone 3 found 3, 2 and 3 MAJOR event-order defects, and each fix (interrupt tickets, held completions, deferred terminal facts) produced new ones; the third review showed that an interrupt used the reader's state and not the state the user sees. With one clock those defects cannot occur. The daemon keeps the property: its session thread is the handler. Translation and parsing stay off the handling thread.
  Date/Author: 2026-11-11 / Claude, integrator. The owner delegated this choice.

- Decision: repeat reviews continue the saved reviewer session and cover only the change since its last verdict. The first review of a milestone starts a fresh saved session (no `--ephemeral`); the session id goes into `.agents/reviewer_sessions.json`.
  Rationale: the owner asked for this on 2026-11-12. Six fresh full reviews of milestone 3 each rebuilt the whole context and repeated all earlier probes.
  Date/Author: 2026-11-12 / owner.

- Decision: dropping a backend client never waits for a thread that can be blocked on a pipe. Teardown stays inside the existing bounded shutdown, as on `main`.
  Rationale: a join on a pipe worker made quit and session switch wait 4.5 s in review 6 while a grandchild held a pipe. Carlos must never feel slow.
  Date/Author: 2026-11-12 / Claude, integrator.

- Decision: reviews run on a Claude subagent, Opus 5.5 at medium effort, not on Codex. Codex remains the implementation worker. Repeat reviews continue the same reviewer with a delta brief.
  Rationale: owner's instruction of 2026-11-12 ("codex is no good at reviews").
  Date/Author: 2026-11-12 / owner.

- Decision: approval consent is bound to the card id of the exact request, and the session alone decides approval identity (`ApprovalState::change` is the one write point; code outside `src/session/` passes drafts or ids, never cards). This is stricter than the pre-plan code.
  Rationale: moving the "first accept" flag to the view lost a reset and let consent carry over to another request after a Claude session switch. The daemon will receive approval answers from several clients, so the session must not trust a caller for identity. In hindsight the stricter guarantee belonged in the daemon plan; it cost three review rounds here.
  Date/Author: 2026-11-12 / Claude, integrator.

- Decision: the common complete-history loader rejects a history whose thread id differs from the active thread and leaves the session unchanged. Fork and switch keep their explicit paths, which adopt the thread and its keyed draft together. At startup the error goes through the existing startup error path (print and exit).
  Rationale: adopting rows of another thread without adopting the thread leaves rows that live updates cannot reach. The reviewer traced every caller: no real start, resume, reconnect or rollout-file fallback can deliver a foreign history, so a start that works today cannot fail because of this. This differs on purpose from the pre-plan code, which adopted the foreign rows.
  Date/Author: 2026-11-12 / Claude, integrator.

### Entry identity and history rules


Create `src/session/entry_id.rs` and `src/session/transcript.rs`. `EntryId` is a private-value newtype with equality, hashing, copying, and debug support. Store a Carlos-generated namespace and a checked increasing `u64` counter. Never silently wrap the counter or derive it from transcript length. The store owns allocation. A temporary history builder produces entry drafts without allocating public ids; `replace_history` allocates or retains ids only when it adopts the completed draft. Renderer-only copies retain the source id and do not enter the store.

The ordered store keeps `Vec<Message>` and a private `HashMap<EntryId, usize>`. It also keeps `HashMap<BackendEntryKey, EntryId>` and `HashMap<String, EntryId>` for per-turn aggregate diffs. `BackendEntryKey` is a typed internal key, not the public id. It includes backend kind, logical conversation identity, the native turn/item identity when available, and the display part. Keep the distinction between an item called `x#part-1` and part 1 of item `x`; do not recreate that distinction with ambiguous string suffixes. Group turn-scoped Codex keys exactly as `item_key` does today. Preserve backend native ids unchanged when replying or sending rewind requests.

Apply these rules in every entry producer:

- A placeholder receives an id on first insertion. Deltas, completion, role changes, file-path updates, diff updates, and later AI labels keep that id. Duplicate started/completed messages do not allocate another entry.
- Each displayed diff part has its own id. An existing `(native item, part)` keeps its id while that part changes. A newly added part gets a fresh id. An absent part loses its live lookup. Never assign one part's id to another part because their vector offsets happen to match.
- A same-thread complete-history replacement retains an old id only when one exact `BackendEntryKey` identifies the old and new entries uniquely. Corrected text does not change identity. Newly discovered entries get fresh ids. Missing or ambiguous keys get fresh ids; never match text, role alone, or an offset. Dropped ids stop resolving. Replace the maps and vector together after parsing and checks succeed.
- Claude's current generated `claude-msg-<stream>-<message>-<block>` ids are not durable history keys. The backend must attach an internal JSON source key derived from the native assistant message id/UUID and block position, user UUID, or tool-use id, as appropriate. Make live and history paths agree when native evidence exists. For records without that evidence, state explicitly in tests that replacement gets fresh ids. Keep translator-generated ids private to backend correlation.
- A same-conversation truncation keeps ids for the retained prefix, retires the suffix and its maps/bindings, and does not rewind the allocator. Reintroduced dropped work gets a fresh id. Carlos ignores an update whose generation, conversation, or retired-entry binding no longer matches. Existing late-event rules remain in force.
- A successful fork or session switch starts a fresh id namespace for every adopted entry, including copied prefix entries. This covers Codex and Claude forked history and Grok's cloned prefix. A failed history draft or fork request before session adoption leaves old ids and facts intact. Claude adopts its new session at `claude/sessionChanged`, before the replacement helper finishes starting; a later startup failure keeps that new session and its ids and recovers drafts as today. Do not roll the UI back to the old session. A Claude conversation reset retires transcript ids once, preserves process tasks and pending approvals as it does today, and gives subsequent transcript entries fresh ids even before the new native session id arrives.
- Local human echoes, automatic prompts, notices, and id-less history records also get Carlos ids. Bind an echo to a later backend source only through an explicit returned prompt UUID or existing native acknowledgement. Never merge two equal texts. An id-less record may receive a new id on complete replacement; this is an intentional and tested limit, not a promise of identity Carlos cannot prove.
- Read coalescing keeps both stored entry ids. The earlier entry gets the combined label, and the later entry remains an empty hidden shell with its own backend key. A later update to the shell must still find the shell. A compact display copy carries the original id and never requests another id.

`PendingApprovalRequest` is currently separate from `messages`. Give a pending approval a Carlos-owned entry id from the same allocator when Carlos first accepts it, retain it across duplicate delivery and focus changes, and retire it when the request resolves or is cancelled. Do not add an approval transcript row or resolved row in 016. Keep process-scoped approval ids across a Claude clear when the existing callback survives. This prepares future clients to refer to the existing card without changing today's screen.

### Target ownership


At the end, `src/app/state.rs::AppState` contains exactly `session: SessionState` and `view: ViewState`. `src/app/view_state.rs` owns widgets and local display state. `src/session/state.rs` owns session facts, grouped into small structs for transcript, turns, prompts, settings, approvals, backend facts, Ralph, and usage. The app owns the backend handle as it does now; session handlers receive `&dyn BackendClient`. Do not start a new thread or introduce a new runtime.

Only methods implemented under `src/session/` may write session fields. Make those fields private or `pub(in crate::session)`, expose read accessors, and provide explicit methods for commands and backend messages. App handlers may call those methods, but cannot reach mutable fields or obtain `&mut` references to a whole facts struct. No `Deref` trick or flat-field compatibility layer may remain at completion. Rendering reads borrowed facts; it does not clone the transcript each frame.

Each mixed handler becomes two functions where necessary. A view function reads a key or widget and supplies plain text, an entry id, a native request id, or an answer value. A session function performs the backend control or changes facts and returns a synchronous `SessionUpdate`. The view applies that report to its caches and editors. Keep the existing `ServerRequestAction` enum as an internal backend-action list, renamed or moved if useful; it is not a new backend-neutral event enum. Do not pass `AppState`, `ViewState`, `UiEvent`, key codes, colors, rendered rows, or a callback that mutates widgets into the session.

## Plan of Work


Each milestone belongs to one worker. The worker may edit only its listed paths and this living ExecPlan. A file-family pattern below means the named concern and its existing companion modules, not arbitrary repository files. Register new session tests in that session module's own test bridge; keep new files within the code-style limits. Preserve a working call path while moving one concern at a time, then remove its old implementation. Do not carry two independently mutable copies of the same fact.

Every milestone, including the preflight, ends with a successful build, `cargo test`, and `bash scripts/test.sh`. Repeat both recorded perf cases after code changes. A worker must record a missing tool or failed check and leave that milestone incomplete rather than claim it passed. Do not install a binary. The final milestone also runs the release terminal checks and the isolated Codex contract check. Workers may use `rtk proxy` to run the commands without hiding their output.

Milestones 1 and 3 can run in parallel after milestone 0, on isolated checkouts with their own build directories. Their edit sets are disjoint: milestone 1 owns plain/rendered model files and their consumers; milestone 3 owns Claude event translation, backend tests, and the named app bridges. Milestone 3 must not edit `src/app/mod.rs` or `src/tests.rs`; register its tests under `src/claude_backend/mod.rs`. Each worker runs the complete milestone checks in its checkout. One worker combines them, resolves import changes, and reruns all checks before milestone 2 or 4 continues. Within a single shared checkout, run them sequentially. The remaining milestones run in order; overlapping state and test files make parallel edits unsafe. Milestone 2 requires milestones 1 and 3 so that it can use Claude's source keys. This ordering describes when work can run; it does not request more workers during plan authoring.

### Milestone 0: record speed and establish a passing starting tree


The plan author already measured the unchanged source. Commands, build failures, and six reports appear under `Artifacts and Notes`. No captured large session exists under the repository or `benchmarks/`; the only `.jsonl` outside build/dependency directories is `src/tests/fixtures/stale_transcript_replay.jsonl`. It contains usable `response_item` records, so the author measured it too. The synthetic case supplies the large transcript.

The implementing worker finishes the default-feature preflight on a host with `libclang` and `cmake`, runs the mandatory checks, and records counts and any existing failures. The worker may edit only this plan; it must not change source to make the baseline pass. If the worker uses a different host, capture a fresh before-change baseline on that host in addition to retaining these reports. This milestone protects plan 017 from accepting a slower core under an incomparable build or machine.

### Milestone 1: separate plain message data from rendered rows


Create `src/session/mod.rs`, `src/session/models.rs`, and `src/app/view_models.rs`; declare `mod session` in `src/main.rs`. Move `Message`, `Role`, `TurnStatus`, `MessageKind`, `ToolOutputDetails`, `DiffBlock`, and `ThreadSummary` to the session model module. Move `StyledSegment`, `RenderedLine`, and `TerminalSize` to the view module. `src/app/models.rs` may temporarily re-export the plain types to keep call sites building, but must not define or re-export the styled types. Move the renderer-only `ToolSummary { rows }` variant into a view `DisplayKind`; make compact output use a `DisplayMessage` that carries the source `Message` and its id once milestone 2 adds it. Stored messages must never contain terminal row counts. Keep the compact and full screen outputs identical.

Allowed edits: the three new modules, `src/main.rs`, `src/app/mod.rs`, `src/app/models.rs`, `src/tests.rs`, and imports or display-type use in `src/app/{flow_transcript,input,input_dispatch,input_events,input_events_keys,layout_state,mouse_events,overlay_render,overlay_render_claude,overlay_render_model_settings,overlay_render_perf,perf_bench,perf_session,picker_delete_dialog,picker_render,render,render_cache_state,render_input,render_sections,render_status,render_transcript,selection,source_highlight,state_input,state_transcript,terminal_ui,terminal_ui_picker_events,tool_output_state,tool_summary,transcript_block,transcript_body,transcript_diff,transcript_diff_source,transcript_render,transcript_source,transcript_styles,transcript_wrap,transcript_wrap_counting,claude_question_input,claude_question_state,claude_task_view,codex_goal_text,codex_goal_view,codex_panel_view,codex_queue_view}.rs`. Existing rendering tests under `src/tests/` may change only their type/import setup and assertions needed for `DisplayMessage`; exclude `src/tests/claude_backend_tests/**`. Do not edit the Claude event files reserved by milestone 3.

Run the common checks and existing phone, source-color, summary, Markdown, and diff rendering tests through the UI runner. The new plain model module must contain no terminal imports. This milestone gives plan 017 a movable model file instead of a module that mixes `String` data with ratatui `Style`.

### Milestone 2: assign ids at the transcript owner


After milestones 1 and 3, add `EntryId`, `BackendEntryKey`, an entry draft, and the transcript store described above. First route all production insertion paths through that store. Replace `item_to_message_index` with `item_to_entry_id`, and `turn_diff_to_index` with `turn_diff_to_entry_id`. Keep a private id-to-position map for fast access. `item_message_index` may survive temporarily as a private adapter that resolves an entry id; backend maps themselves must no longer store positions. Convert summary bindings, prompt metadata, unknown Claude prompts, and persisted view references to ids rather than saving resolved offsets.

Allowed edits: `src/session/{mod,models,entry_id,transcript,entry_key,history_identity}.rs`, separate tests beside those modules, `src/backend.rs` only for value-type derives such as `Hash` on `BackendKind`, `src/app/{state,state_items,state_transcript,state_thread,state_input,item_history,notification_items,notification_tools,tool_output_items,state_summaries,input_history_state,input_history_native,claude_prompts,claude_session_history,claude_reset,claude_rewind,turn_submit,approval_state,approval_parsing,claude_callbacks,codex_callbacks,codex_questions,perf_synthetic,perf_session,perf_bench,render_cache_state,render_transcript}.rs`, and `src/tests.rs` plus affected tests under `src/tests/`. Splitting a named module into a concern-specific companion in the same directory is allowed. Add deterministic identity checks under `tests/ui/test_entry_identity.py` and its fixed peer fixture; register them in `tests/ui/test_workflows.py`.

Replace `perf_session`'s direct `app.messages = ...`, Grok's raw `messages.push`, and all `Message` literals and test-only truncation helpers with store or draft helpers. Do not give each test row the same fake id. Add invariant tests for allocator uniqueness, every map pointing to a live id, several diff parts, live-to-history matching, corrected history, duplicate native ids in different turns, id-less records, rejected replacement, fork/reset, read grouping, delayed summaries, and local echo binding. The UI identity workflow must assert the unchanged visible text and the exact rewind/control requests; Rust tests assert internal ids. This milestone gives plan 017 one owner for identity and storage with no dependency on vector positions held by clients.

### Milestone 3: emit translated Claude events at the backend boundary


Create `src/claude_backend/event_normalizer.rs` and small companion modules for live lifecycle, controller routing, and history decoding. Let a backend-owned normalizer hold separate live/history `ClaudeTranslationState` values plus the accepted-prompt/result/idle tracker currently in `ClaudeUiState`. Connect it after `ConversationGuard::route` and before `EventSender::send`. Route asynchronous control replies through the same owner; replies to synchronous callers still go to their reply channels. Ensure registration of a submitted prompt precedes native replies, and preserve interrupt rollback when sending the control fails.

Move the native-event decisions from `claude_sdk_events::receive/main_items/status`, the result/idle methods of `ClaudeUiState`, and the translator/chunk-decoding portion of `claude_session_history::append_record/read_chunk` into the backend. Keep source authorship (`human`, `automatic`, `unknown`), UUIDs, child filtering, thinking de-duplication, hidden tool rules, clear/reset guards, usage freshness, and history bounds unchanged. Emit the same common `thread/initialized`, `item/started`, `item/*/delta`, `item/completed`, `turn/started`, `turn/completed`, usage, and approval messages that the session handles for the other backends. Preserve prompt completion after both result and idle, with no synthetic extra turn. Preserve one-item-at-a-time history consumption and the current complete-before-start behavior.

For Claude-only facts such as capabilities, task records, browser completion, quota, prompt acceptance, session changes, and control failures, emit named JSON `method`/`params` messages carrying the needed facts and correlation tokens. Retain backend-specific methods where no common method exists. Do not forward a raw helper `frame`, a `claude/controller` envelope, or an SDK message that requires another translator call. Add source-key metadata to common item messages and normalized history prompt messages so milestone 2 can identify entries by native evidence. Do not claim generated stream counters are durable identity.

Allowed edits: `src/claude_backend/**`, `src/app/{claude_sdk_events,claude_notifications,claude_state,claude_session_history,claude_control_replies,claude_reset,claude_prompts,claude_context,claude_callbacks,claude_task_stops,turn_interrupt,notifications}.rs`, `src/tests/claude_backend_tests/**`, and Claude backend test files within `src/claude_backend/`. The app bridge files may apply normalized facts and request the next history page; they must no longer translate native events. Do not edit `src/app/mod.rs`, `src/tests.rs`, or the milestone 1 paths. Keep `src/backend.rs`'s trait unchanged.

Extend the backend's receiver-level tests so a caller that constructs no `AppState` can observe streamed reasoning, text, tool output, history, approvals, and exactly one final turn completion. Test result-before-idle, idle-before-result, errors, interrupts, duplicate snapshots, late old-generation frames, clear/init, and queue overflow. Compare the complete ordered method/params records with fixed expectations. Run the existing Claude terminal suites with the compiled controller. This milestone gives plan 017 a self-contained Claude backend; it must not ask a future TUI to translate SDK frames.

### Milestone 4: introduce the two owners and move transcript and turn writes


Create `src/session/state.rs`, `src/session/turns.rs`, `src/session/transcript_updates.rs`, and `src/app/view_state.rs`. Put thread ids/names, active turn, transcript store, item/turn maps, labels from parsed commands, turn status, Ralph state, approvals, and context counts under the session owner. Put composer, escape chord, viewport, layout, render caches, tool-fold preference, dictation, and performance counters under the view owner. Move the transcript and turn-changing methods from `state.rs`, `state_items.rs`, `state_transcript.rs`, and `state_thread.rs` into session methods. Keep view rendering/selection methods in app modules. Mixed child structs may remain as explicitly temporary `AppState` fields until their named milestones; do not put a widget into `SessionState` as a shortcut.

Session methods return the earliest changed entry id, or a whole-transcript reset flag, through `SessionUpdate`. The view resolves that id once and calls the existing incremental `mark_transcript_dirty_from`. Preserve dirtying of the preceding read-summary row and predecessor-dependent spacing. Replace the turn's saved start index with an internal turn boundary that can represent an empty turn and identify its first appended entry. Ralph must scan the same message suffix as before, including when no entry existed at turn start. Keep allocator counters separate from this boundary.

Allowed edits: `src/session/**`, `src/app/view_state.rs`, `src/app/state*.rs`, `src/app/{mod,render*,overlay_render*,input*,mouse_events,claude*,codex*,notification*,item_history,turn_*,native_activity,session_name,tool_*,summary_request,perf*,picker*}.rs`, and `src/tests.rs`/`src/tests/**` for mechanical `app.session`/`app.view` reads, constructor changes, and adapters. This broad caller pass changes access paths only outside the named transcript/turn methods. Do not change backend wire behavior or renderer algorithms. New test setup belongs in named helpers, not repeated struct literals.

After the common checks, compare the hidden-row rewind screen and read grouping with the previous milestone. Test a delta that changes an earlier grouped row and an empty interrupted turn. This milestone gives plan 017 an actual session owner whose transcript and turn methods accept no `AppState`.

### Milestone 5: move shared facts and remove hidden renderer dependencies


Split `RuntimeSettingsState`: backend capabilities, current/pending model/effort/summary, and available models go to `src/session/runtime_settings.rs`; overlay visibility and every `model_settings_*` field go to the view. Move plain `RuntimeDefaults` out of `app/mod.rs` into the session settings types. Pass startup preferences explicitly. Keep file persistence and editing in the app. Split `InputHistoryState` as detailed in the inventory: session prompt records and native positions use `EntryId`; view navigation stores its selected entry id and restored drafts. Keep native positions for user items without visible text as `NativeUserHistory` currently does.

Move `SummaryState` and `state_summaries` fact-changing methods into session modules, preserving job cancellation and the original-label fallback. A delayed result must match both the current entry binding and summary key before it changes a label. Move plain command/diff/source classification helpers out of renderer modules: read-summary parsing from `transcript_render`, `source_path_from_item` from `transcript_source`, `diff_summary` from `tool_output_state`, and `split_file_diffs`' plain parser from `transcript_diff_source` if it currently shares styled imports. Move the terminal-free shell/tool helpers with their dependencies. Keep wrapping, row counts, syntax colors, and `DisplayMessage` in the view.

Move `RateWindow`, quota arithmetic, and clock helpers out of `weekly_usage.rs` into `src/session/usage.rs`; let `weekly_usage` map a plain pace category to the same colors. Keep context/weekly text unchanged. Keep backend read deadlines in session state and the next purely visual countdown redraw in view state. `RateLimits::next_draw` currently combines them and must be split without losing wakeups.

Allowed edits: `src/session/**`, `src/app/{runtime_settings*,state_settings,state_summaries,summary_request,input_history*,state_input,tool_command_view,tool_output_items,tool_output_state,tool_output_preferences,tool_output_command,tools,tool_format*,tool_diff,tool_shell*,shell_*,transcript_source,transcript_diff_source,transcript_render,weekly_usage,rate_limits,context_usage,state,view_state,mod,cli,backend_setup*,input_timers,render_status,overlay_render_model_settings,input_events_model_settings}.rs`, and affected shared settings/history/tool/summary/usage tests under `src/tests/`. Existing `src/summaries/` may change only its entry-binding type or imports. Keep preferences file paths and contents compatible.

Run the common checks and verify that an AI label arriving after replacement cannot alter a different entry. Exercise model settings, pending human prompts, read grouping, source colors, directory labels, and quota countdowns through existing tests. This milestone gives plan 017 plain shared helpers whose imports cannot lead back to a renderer.

### Milestone 6: separate Claude facts from local Claude UI state


Move the remaining Claude phase, accepted prompts, quota/context reads, process activity, tasks, reset guards, capabilities, backend model choices, history source records, browser-request facts, and fork progress into `src/session/claude/`. The backend normalizer from milestone 3 owns translation and result/idle bookkeeping; do not copy them back. Split `ClaudeSelection`: inherited model/effort facts belong to the session; the saved settings editor values in `editing` belong to the view.

Keep task-panel visibility, scroll, selected task, unknown-source rewind focus, confirmation focus, and unsent recovery drafts under `ViewState`. A submitted edited prompt and its in-flight UUID belong to the session until accepted or failed; a session result can return recovery text to the view without replacing a newer composer. Preserve the existing notices that let F5 recover queued text. Move `claude_edit::label` into a view-side helper. Separate browser consent/completion facts from the editor notice shown before consent. Do not launch a browser from the session.

Allowed edits: `src/session/**`, `src/app/claude*.rs`, `src/app/{state,view_state,mod,state_input,state_settings,turn_submit,turn_interrupt,turn_approval,notifications,native_questions,input_mode,input_timers,input_events_keys,input_dispatch,overlay_render_claude,render_status}.rs`, and Claude tests under `src/tests/`. `src/claude_backend/**` may change only exports or source-key access needed by the session bridge, not its milestone 3 behavior. Keep existing Claude UI fixtures and extend them if a failing workflow exposes a missing case.

Run the common checks. Prove that process tasks and callbacks survive a clear, older context reads cannot restore counts after compaction, and a failed asynchronous edit preserves a newer draft. This milestone gives plan 017 Claude session methods that need neither a composer nor a task panel.

### Milestone 7: separate callback records from answer editors


Move `ApprovalState`, approval parsing, typed native callback ids, callback queues, payloads, blocking flags, reply/validation tokens, retired-id windows, and callback byte budgets into `src/session/approvals/` and `src/session/codex/callbacks.rs`. Split `CodexCallback`: remove `draft: NativeDraft`, keep `params`, `blocking`, `revision`, `candidate`, `checking`, `interrupting`, and `bytes` in session facts. The candidate is a submitted answer under validation, not an editable widget. Keep callback focus and which nonblocking callback the user opened in the view.

Move `NativeDraft`, `QuestionViewport`, `AnswerField` editors/cursors/selected options, `confirm_allow`, and draft-specific errors/notices to the view, keyed by the exact typed callback identity. Split plain question/form descriptions and validation helpers from the functions that construct editors. Session replies accept a plain `serde_json::Value` answer and choice. A late validation reply must still check request id, revision, and current candidate before sending. Retire only the editor belonging to the retired callback; keep other drafts.

Allowed edits: `src/session/**`, `src/app/{approval_*,codex_callbacks,codex_callback_route,codex_reply,codex_questions,codex_state,claude_callbacks,claude_browser,claude_question_*,native_questions,turn_approval,state,view_state,mod,input_events_keys,input_dispatch,overlay_render_claude}.rs`, callback/question tests under `src/tests/`, and fixed callback scenarios under `tests/ui/`. Keep answer conversion and default-to-no confirmation behavior byte-for-byte compatible where tests expose it.

Run the common checks. Test two simultaneous callbacks with separate drafts, one cancellation, a stale form check, a callback after root-turn completion, and a clear with an unresolved process callback. This milestone gives plan 017 callback handling that cannot indirectly import `TextArea` through an answer type.

### Milestone 8: separate Codex queues and goals from their editors


Move queue knowledge, revision, native entries, reads, operations, and uncertain submitted payloads into `src/session/codex/queue*.rs`. Keep queue mode, visibility, selection, `QueueEdit`, recovery presentation, and local messages in the view. An uncertain operation retains the submitted text in the session until the backend settles it; hand recovery text to the view once without resending it. Extract `tick/page/mutate` and reply parsing from `codex_queue.rs`/`codex_queue_receive.rs`; app `command/open/begin_edit/save_edit/operate` read widgets or selection and call those session methods with plain values.

Move goal knowledge/value/revisions, request correlation, read deadlines, accepted changes, and Ralph checks into `src/session/codex/goal*.rs`. Keep `GoalDraft`, `GoalConfirm`, field focus, visibility, scroll, and local edit notices in the view. Preserve revision checks, pending-write guards, native-queue precedence over automatic work, and explicit confirmations.

Allowed edits: `src/session/**`, `src/app/{codex_queue*,codex_goal*,codex_goals,codex_state,state,view_state,mod,turn_submit,input_mode,input_queue_rules,input_events_keys,input_dispatch,input_timers,overlay_render,render_status,state_ralph}.rs`, queue/goal/Ralph tests under `src/tests/`, and existing queue/goal fixed UI fixtures. Do not change which backend calls the user triggers.

Run the common checks. Verify stale queue edits preserve text, uncertain writes do not resend, an authoritative nonempty native queue blocks Carlos automatic work, and a goal overlay preserves its editor while backend facts refresh. This milestone gives plan 017 queue and goal controllers that accept plain requests instead of widgets.

### Milestone 9: separate Codex settings, tasks, and reconnect state


Move Codex connection state, generation fences, held-event journal and byte accounting, partial-stream readers, runtime setting requests, task ownership, panel readers, and goal/queue snapshot installation into `src/session/codex/`. Keep panel visibility/selection/scroll/detail, settings editor drafts, rejected-editor restoration, and redraw effects in view helpers. Split `SettingsState` and `PanelState` according to the inventory below. Preserve every existing deadline, scope check, item overlap rule, and bounded replay window.

Move `codex_history.rs` and `codex_rollout.rs` into session history modules. Change `codex_reconnect_snapshot::reconnect_panel` to build a plain panel reader, not `AppState`. Change `src/codex_snapshot.rs` to call session history/panel helpers directly and remove `app::reconnect_history`/`app::reconnect_panel` exports. Move reconnect action dispatch out of its dependency on `app::input_dispatch`; only the view-specific `finish_ui` effects remain in the app. Keep acknowledgement of every charged event exactly once, including stale, held, replayed, overflow, and fatal-close paths.

Allowed edits: `src/session/**`, `src/codex_snapshot.rs`, `src/app/codex*.rs`, `src/app/{state,view_state,mod,state_settings,input_dispatch,input_timers,input_mode,input_queue_rules,notifications,turn_submit,render_status,overlay_render_model_settings}.rs`, reconnect/task/settings tests under `src/tests/`, and existing reconnect fixed peers under `tests/ui/`. Do not change `src/protocol*.rs` wire methods or refresh the schema merely to complete this refactor.

Run the common checks and `bash scripts/test.sh --codex-contract`. Verify an off-thread reconnect reader constructs no widget, a failed snapshot leaves old facts intact, the same-thread replacement retains known ids, and late callbacks/partial output cannot bypass the current connection fence. This milestone removes the backend-to-app cycle that would otherwise block moving files into `carlos-core` in 017.

### Milestone 10: finish session handlers and enforce the terminal boundary


Move the remaining backend-message dispatch, item/history handlers, turn submission/steering/interrupt/reply logic, Ralph continuation checks, session-name controls, summary updates, and backend timers under `src/session/`. Split `notifications.rs` so key predicates and animation helpers stay in a view module while server handling moves. Split `input_dispatch.rs`, `input_timers.rs`, and `input_queue_rules.rs` so terminal-event ordering stays in the app, but session queue eligibility and backend actions live in the session. Keep the current loop ordering and input/idle budgets. Preserve the one-session process and current process lifetime.

View adapters obtain a rewind target as `EntryId`, supply its edited text, and receive a send/fork outcome. The session resolves native positions and builds `RewindForkRequest`. App code must not calculate backend keep/drop counts from local editor focus. Keep composer clearing, restored draft handling, default-no confirmation, and overlay focus on the view side. Remove temporary app state fields and all migration re-exports of session handlers. `AppState` now has exactly the two owners.

Add `scripts/check-session-boundary.py`, its separate tests under `tests/architecture/`, and a call near the start of `scripts/test.sh`. Implement the mechanical checks specified below. Limit future-core imports to the module roots listed under `Interfaces and Dependencies`. Allow read-only references to facts in renderers; forbid mutable session-field access outside `src/session/` through Rust privacy. Keep the session public API free of terminal types. Include default-feature and no-default-feature compilation in final checks; do not add a crate just to enforce the boundary.

Allowed edits: `src/session/**`, `src/app/{state,view_state,mod,notifications,notification_*,item_history,native_activity,turn_*,state_ralph,ralph*,session_name,input,input_dispatch,input_timers,input_queue_rules,input_events_submit,input_events_keys,backend_setup*,cli,perf*,render*,overlay_render*}.rs`, obsolete app session-only files named in this plan when deleting or replacing their bridges, `src/main.rs`, `src/tests.rs`, affected tests under `src/tests/`, `scripts/check-session-boundary.py`, `tests/architecture/**`, `scripts/test.sh`, and fixed terminal workflow tests under `tests/ui/`. `Cargo.toml` may change only to register an explicit check/test target if needed; it must remain one package with the existing dependencies.

Run the common checks and the new guard tests. Create a session directly in a Rust test, replay common JSON from all three backend receiver fixtures, send plain commands, and inspect facts without creating `AppState` or a terminal. Existing terminal tests must still show the same screens and backend requests. This milestone lets plan 017 move the session directory and listed dependencies without rewriting state or handler code.

### Milestone 11: prove the complete split and record review


One worker runs the final commands below, reviews the entire diff against the three outcomes and the inventory, and fixes defects within the earlier milestone's edit set before rerunning the affected checks. Run the fresh read-only engineering reviewer described in `.agents/reviewers/engineering_reviewer.md` against the concrete change range, invariants, test results, and perf reports. Use a bounded non-interactive review session and copy its findings and verdict into a clearly labeled review paragraph in this document. `FAIL` blocks completion. Do not run native inference or read personal sessions for this refactor.

Allowed edits: this plan and fixes within the preceding milestones' named paths. Do not install Carlos. Record the final source revision, commands, counts, speed comparisons, and remaining limits. All three outcomes must pass together. This milestone gives plan 017 measured evidence and a checked file boundary, rather than an assertion that the split is ready.

## Concrete Steps


Use the repository root as the working directory. `~/.codex/RTK.md` asks for the `rtk` prefix; `rtk proxy` preserves tool output. Search code with `rtk proxy semeja search "intent or symbol" .`, use `semeja find-related <path> <line>` from a result when useful, and use `rg` for exhaustive literal checks. Do not install packages or binaries as an unrecorded workaround. The plan author's exact build attempts were:

    rtk cargo build --release
    rtk proxy env WHISPER_DONT_GENERATE_BINDINGS=1 cargo build --release
    rtk proxy cargo build --release --no-default-features

The first two failed for the environment reasons recorded below. The third printed `Finished release profile [optimized] target(s) in 1m 06s`. The environment override applied only to its one failed command. Do not require it on a host that has the native tools.

At the end of each milestone, run these checks on the integrated tree:

    rtk proxy cargo build
    rtk proxy cargo test
    rtk proxy bash scripts/test.sh
    rtk proxy cargo build --release --no-default-features
    rtk proxy ./target/release/carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48
    rtk proxy ./target/release/carlos perf-session src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48

Run each perf command three times serially with no build or test process competing for CPU. Expect the same transcript counts as below, zero command failures, passing Rust tests, and a passing packaged-terminal run. Record actual test counts after the preflight; the author did not run tests under the restricted plan-writing task and does not invent a count here. A default-feature build must pass even though the speed comparison deliberately uses the same no-dictation profile as the recorded baseline.

At final acceptance, run these additional checks. The last release script rebuilds the default-feature binary, so do not compare that executable to the no-default-feature baseline without first rebuilding with matching flags.

    rtk proxy python3 scripts/check-session-boundary.py
    rtk proxy python3 -m unittest discover -s tests/architecture -p 'test_*.py'
    rtk proxy cargo test --no-default-features
    rtk proxy bash scripts/test.sh --codex-contract
    rtk proxy bash scripts/test.sh --release

`scripts/test.sh` already runs `cargo fmt --check`, builds Rust and the compiled Claude controller, packages them, and runs the deterministic terminal workflows. `--release` additionally runs clippy and release tests. The script creates artifacts under `target/ui-artifacts/`. The Codex contract option uses an isolated generated session without model inference. If a worker actually changes which Codex fields/methods Carlos consumes, follow AGENTS.md's schema rule and treat that change as an explicit scope decision; this plan intends to preserve those fields and methods.

## Validation and Acceptance


Outcome A passes when every stored transcript message and accepted pending approval has a non-reused Carlos id, all backend item and turn-diff maps hold `EntryId`, and no client-facing reference holds a vector offset. Rust tests must assert the exact id relationships from the history rules, not merely check that ids differ from backend strings. Run the new session identity tests and the complete existing Rust suite. Run a fixed terminal workflow that receives streamed text, a completed tool, a history reload, a delayed label, and a rewind. Assert both the visible transcript and the backend's exact fork/submit/reply requests. Keep existing fixture expectations intact unless the change only adds internal ids to an inspected structure.

Test equal prompt texts with different UUIDs; reused Codex item ids in different turns; literal ids containing display-part-looking suffixes; a new diff part; a missing old part; same-thread insertion before an existing entry; late updates after truncation/fork; duplicate clear events; failed history parsing; and an empty read shell receiving a later update. Assertions must prove that no stale id points to an unrelated row and that known same-thread entries retain their ids despite changed offsets or corrected text. Allocate ids through the same constructor in performance fixtures and test helpers as in production.

Put focused identity tests in `src/session/transcript_identity_tests.rs` and companion files below 400 lines. Require `same_thread_reload_preserves_ids_after_insertion`: build source keys A/B, reload X/A/B with corrected B text, and assert A/B retain ids while their offsets change. Require `fork_remints_prefix_and_retires_old_ids` for all three backend paths, `truncate_never_reuses_a_removed_id`, `read_coalescing_keeps_the_hidden_shell_id`, `diff_parts_do_not_alias_literal_suffix_ids`, and `stale_summary_cannot_write_to_a_replacement_entry`. Put callback-id lifetime tests in the approval test module. These tests fail against the old index-only design and pass only when the store enforces the specified relationships. Add `src/session/session_headless_tests.rs::common_messages_need_no_view` to exercise the final session API with no app fixture. Register the fixed terminal scenario as `test_entry_identity.py::test_reload_then_rewind_preserves_screen_and_native_target`; its backend request assertions must identify the selected native message, not compare only visible prose.

Outcome B passes when `AppState` has only `session` and `view`, the inventory's session facts live under `src/session/`, and code can construct and drive `SessionState` with no app or terminal. The script at `scripts/check-session-boundary.py` must do more than search `state.rs`. It must enumerate all `.rs` files under the future-core roots listed below, follow module declarations including `#[path]`, and include their test modules. Add an explicit list of existing external test paths reached by those declarations, such as `src/tests/protocol_history_tests.rs`, rather than allowing the entire terminal-heavy `src/tests/` tree. Reject any reachable local file outside those roots and the listed tests. In those external tests, reject imports from the app test bridge and use terminal-free test support. No excluded test may hide a production dependency.

The script rejects terminal crate identifiers in imports, paths, aliases, re-exports, macro paths, and extern declarations. Its minimum forbidden set is `ratatui`, `ratatui_core`, `ratatui_interact`, `ratatui_textarea`, `crossterm`, `ansi_to_tui`, `tui_markdown`, `termion`, and `tui`. Also reject `crate::app`, `crate::event`, `crate::theme`, `crate::clipboard`, and `crate::dictation` paths and any re-export through those roots. Derive and review the external-crate allowlist from Cargo's dependencies, with only the nonterminal dependencies listed below admitted. Fail closed for a new external dependency until the list is reviewed. Ordinary raw strings containing output control bytes are data, not terminal-crate dependencies; lex Rust comments/strings separately so those do not create false positives. Ban `include!` or unchecked external module paths in this tree rather than providing an escape around the scan.

Give the guard tests in-memory Rust samples that import a forbidden crate directly, alias it, re-export it, reach it through another local module, and hide it in a `#[path]` child. Each must fail and name its source file. A clean sample and the real tree must pass. Add a static check that no `impl SessionState` lives outside the session tree and no session module accepts an app/view/widget type. Rust field privacy supplies the stronger check against app code writing session fields. At the end, `rtk proxy rg -n 'crate::app' src/codex_snapshot.rs src/claude_backend src/grok_backend src/summaries src/session` must have no matches. These checks complement compilation; plan 017 supplies the actual Cargo dependency boundary when it adds the core crate.

Outcome C passes when a test reads `ClaudeClient`'s receiver without an app, sees the common JSON item/turn/usage/approval shapes, and sees no raw helper frame requiring interpretation. Backend-only facts may use named extension methods with the same JSON `method`/`params` envelope. Codex's existing generation wrapper and native control extensions remain compatible; this plan does not redesign all backend extensions. A mechanical search must find no calls or imports of `translate_claude_line`, `ClaudeTranslationState`, or `begin_history_snapshot` under `src/app/` or `src/session/`, and no handling of `claude/controller` there. Assert source keys and generation/epoch metadata on normalized Claude messages and verify that malformed/overflow events still stop the owned helper through the existing failure path.

The speed gate compares transcript counts first, then elapsed replay time, apply time, full layout/draw, and p50/p95 draw/append times on the same host and build flags. The three synthetic baseline runs give a replay median of 53.38 ms, full-layout median of 17.59 ms, full-draw median of 4.16 ms, and p95 medians of 0.99 ms scroll, 0.79 ms typing, 0.77 ms working, and 0.92 ms append. The small fixture gives a replay median of 1.33 ms and full-draw median of 0.63 ms. All new ids and map costs are included; do not bypass the entry store in the synthetic builder to conceal them.

Any repeatable slowdown blocks acceptance under the agreed design. If a metric rises by more than 5% or 0.10 ms, whichever is larger, collect another three before/after runs under the same conditions to distinguish host noise from code cost. That trigger is not an allowed regression budget; smaller repeatable regressions also need correction. Compare maximums and overall allocation growth during review, but do not treat the broken zero-memory fields as evidence of constant memory. Never scan or rebuild the whole transcript per delta or keypress. Retain the bounded caches and existing incremental redraw behavior.

### Risks and concrete mitigations


- Render caches and scroll anchors currently save vector positions. Keep their contiguous vectors for speed, carry an entry id at the persistent anchor, invalidate on the earliest changed entry, and rebuild the view after replacement. Test scroll/selection during streaming, resize, hidden rewind rows, and read coalescing.
- A function may need both state halves and fail Rust's borrow checks. Pass `&mut session` and `&mut view` as distinct borrows at the adapter, take a small owned command before calling a session method, and apply its report afterward. Do not add `RefCell`, shared mutexes, unsafe pointers, or transcript clones to avoid explaining ownership.
- Claude's current app filters translator output and supplies native completions itself. Move both rules together, keep one lifecycle owner, and compare receiver records and terminal screens for result/idle permutations, interrupts, child messages, clear, and history.
- The Claude normalizer must coordinate prompt registration with the stdout reader. Register the prompt before writing it, release its lock before a synchronous helper call or queue send, and roll back interrupt intent on a failed control. Keep all resulting messages in the same order as the accepted native frames; do not add a competing translator thread.
- The event queue charges bytes until the consumer handles a line. Stamp and charge each emitted normalized line, acknowledge its actual delivered length exactly once, and preserve the 1024-frame/8 MiB failure behavior. Test fan-out from one native frame, partial enqueue failure, old-generation drops, and reconnect-held frames.
- A history builder may allocate ids before adoption or reset counters during rewind. Build drafts, reconcile once at adoption, and test failed builds, retained prefixes, forked clones, and non-reuse. Keep only live source-key/id maps; do not grow an unbounded registry of every retired entry.
- A delayed label or form validation can target a retired entry/request. Match id plus binding/request revision and content key, cancel old jobs on replacement, and ignore stale results. Do not fall back to old positions or matching prose.
- `src/tests.rs` and many test files construct `AppState` or literal `Message` values. Provide one deterministic session fixture and one view fixture; move session tests away from the terminal-heavy bridge. Update performance builders too. Tests must not preserve the dependency that production code is removing.
- Helpers with harmless names import rendering indirectly. Move plain parsing out of weekly colors, transcript/source rendering, diff layout, question editors, and preferences. The guard scans the complete permitted module set, including backends and snapshot helpers, and rejects app re-exports.
- Recovery text belongs to different actors at different times. The session owns a submitted request until its backend settles it; the view owns a fresh unsent draft. Return failed/uncertain text explicitly once, preserve a newer draft, and retain existing F5 notices and no-resend behavior in fixed UI tests.
- A large mechanical caller pass can obscure behavior changes. Move one concern per milestone, keep every intermediate tree buildable, preserve backend request/screen assertions, and review the semantic methods separately from import/access-path edits.

## Idempotence and Recovery


The benchmark opens only the named repository fixture or generates synthetic text and draws to ratatui's in-memory test terminal. It makes no backend model call. Run it repeatedly with the same flags. Cargo writes build artifacts under `target/`; it does not install Carlos. The author created only this ExecPlan as repository source and made no commit.

Implement one milestone on top of a passing tree. If a gate fails, keep that milestone open, preserve the failing fixture/artifacts, and repair it before proceeding. Do not delete unrelated work or reset a shared checkout. Move files with their tests and retain thin read-only re-exports only for the stated intermediate steps. Remove those bridges in milestone 10. A failed parse or rejected fork before adoption must not partially replace live entries or consume their ids. Preserve Claude's existing adoption point and post-adoption startup-failure behavior. Session resets retire live ids and bindings atomically; they do not recycle counters.

Do not install anything to `~/.local/bin/carlos`, run `git -A`, add a coauthor trailer, or commit during this planning task. During later implementation, keep any commit/review decisions with the human coordinating the plan. Before that human accepts 016, record all evidence in the living sections and leave the release binary in `target/` only.

## Artifacts and Notes


### All current AppState fields and their owners


This inventory comes from `src/app/state.rs::AppState` at the revision recorded below. Each bullet assigns one of the 29 current fields. A split field names the session facts first and the view-owned children second, rather than pretending its current wrapper has one owner.

- `thread_id: String`: session, under thread facts.
- `thread_name: Option<String>`: session, under thread facts.
- `active_turn_id: Option<String>`: session, under turn facts.
- `messages: Vec<Message>`: session, under the transcript store, with `EntryId` on every stored entry.
- `render_cache: RenderCacheState`: view, including rendered blocks/counts/offsets, compact copies, block plans, hidden-row offset, width, theme, and dirty positions.
- `tool_output: ToolOutputState`: view, including mode and resize hysteresis (`auto_compact`). Plain diff-label helpers move separately to the session.
- `summaries: SummaryState`: session for `enabled`, `cwd`, `bindings`, `pending`, `service`, and `replaying`; view for `draft`. Bind by entry id and key, not an offset.
- `layout: LayoutState`: view, including active and preferred transcript themes and terminal size.
- `item_to_message_index: HashMap<String, usize>`: session, replaced by backend source-key-to-`EntryId` maps. Only the store's private id-to-offset map may map entry identity to storage positions; the view's render-cache positions remain disposable.
- `completed_item_ids: HashSet<String>`: session, retaining native completion keys and their turn/part scope.
- `turn_status_by_id: HashMap<String, TurnStatus>`: session.
- `turn_diff_to_index: HashMap<String, usize>`: session, renamed to `turn_diff_to_entry_id` and storing ids.
- `command_render_overrides: HashMap<String, String>`: session, as parsed tool-label facts despite the current name.
- `input: TextArea<'static>`: view, the main composer.
- `input_history: InputHistoryState`: session for submitted prompt records (`input_history` and their optional entry ids), `turn_ids_by_message`, and `native`; view for `input_history_index`, `input_history_draft`, `rewind_mode`, and `rewind_restore_draft`. A view may retain a pending unsent history draft; the session retains accepted/queued input metadata. Native positions stay separate from visible entries and keep their current deduplication rules.
- `esc_armed_at: Option<Instant>`: view.
- `status: String`: session for backend/turn/Ralph status; view gets a separate local hint/error string. Preserve today's visible precedence with a view helper.
- `turn_start_message_idx: Option<usize>`: session, replaced with an internal turn boundary capable of an empty turn, not a client-visible index. The boundary records the same suffix for Ralph marker checks.
- `ralph_runtime: RalphRuntimeState`: session, including queued inputs, origins, deadlines, markers, prompt path, and waiting/toggle state.
- `dictation: DictationRuntimeState`: view, including phase, endpoint/profile choice, picker, and disabled reason.
- `dictation_io: DictationCaptureState` behind `cfg(feature = "dictation")`: view, including capture/worker handles, audio, request counters, and cancellation.
- `runtime: RuntimeSettingsState`: session for `supports_effort`, `supports_summary`, `current_model`, `current_effort`, `current_summary`, `pending_model`, `pending_effort`, `pending_summary`, and `available_models`; view for `show_model_settings`, `model_settings_field`, `model_settings_tool_output`, `model_settings_model_input`, `model_settings_model_index`, `model_settings_effort_options`, `model_settings_effort_index`, `model_settings_summary_options`, and `model_settings_summary_index`.
- `approval: ApprovalState`: session for pending request facts, offered choices, native id, and Carlos entry id. Focus, confirmation, answer drafts, and editor scroll belong to view structs described below.
- `claude: ClaudeUiState`: split into backend-owned translation, session-owned Claude facts, and view-owned Claude panels/editors as detailed below. Do not move the wrapper intact.
- `codex: CodexUiState`: split into session-owned Codex facts and view-owned Codex panels/editors as detailed below. Do not move the wrapper intact.
- `viewport: ViewportState`: view, including scroll/follow state, selection, mouse tracking, help, local steer notice deadline, and scroll direction.
- `context_usage: Option<ContextUsage>`: session.
- `rate_limits: RateLimits`: session for `windows`, `during_read`, `pending`, and `next_read`; view for `next_draw`. Plain weekly counts/pace move out of the styled weekly indicator module.
- `perf: Option<PerfMetrics>`: view, because it counts terminal-loop events, layouts, and draws. Benchmark timing code stays app-side and exercises session APIs.

### Mixed Claude and Codex children


`ClaudeUiState` session fields are `phase`, `enabled`, `epoch`, `notices` (the existing bounded duplicate-notice keys), `quota`, `reset_key`, `awaiting_reset_init`, task records, `activity`, `context`, accepted `prompts.entries`, `generation`, native history records/unknown-source metadata/chunk progress, `capabilities`, `observed_model`, queued `questions`, and inherited model/effort choices. Move `translation` and native `pending`, `cycle`, `results`, `interrupted`, and `idle` completion bookkeeping to the backend normalizer, along with history translation/chunk decoding. Keep any page-progress token needed by the session as plain facts, without a translator reference.

Claude view fields are `confirm_allow`, `answers.drafts`, `selection.editing`, `prompts.queue` (the user's local queue/steer choice), task `show`/`scroll`/`selected`, history `unknown_selection`, and unsent recovery text. `history.edit` must split: accepted fork/send progress and UUID correlation are session facts; failed-state display, a still-unsent `edited_prompt`, and `unsent_drafts` are view state. Move a submitted edited prompt to session ownership while in flight. `NativeAnswers.browser` contains consent/completion facts and belongs to the session, despite sitting beside drafts today. `ClaudeTasks` keeps records/live/runs/run order/sequence/bytes/stops/control failures in the session. `NativeDraft` and `QuestionViewport` remain in the view; derive their schema/notice state from session callback facts.

`CodexUiState` session fields are `closed`, `item_turn`, `item_part`, `connection`, `enabled`, `epoch`, `serial`, and `task_status`, plus the fact portions of `callbacks`, `queue`, `settings`, `panel`, and `goals`. `item_turn`/`item_part` remain temporary native-key scope during one handler, not public entry identity. Every field of `ConnectionState`, its `ReplayFrame`, `MetadataRead`, and `Streams` belongs to the session, including socket path, rebuild flags, held bytes, and generation fences. Keeping those facts does not introduce a new socket.

For `CodexCallbacks`, keep `queue`, `entries`, `recent`, and `retired` in session facts; move `focused` and `opened` to the view. In each `CodexCallback`, keep every current field except `draft` in the session. Move its `NativeDraft` to a typed-request-keyed view map. A form's candidate/revision/checking flag tracks an already submitted answer and must remain in session facts.

For `QueueState`, keep `known`, `revision`, `entries`, `read`, and `operation` in the session. Move `mode`, `show`, `selected`, `edit`, and an offered recovery draft to the view. Preserve backend read/write `error` and acknowledged `notice` as session facts; separate local edit errors and hint text in the view. `QueueRead` and all of `QueueOperation`, including `submitted`, belong to the session while pending. `QueueEdit` belongs wholly to the view, including `id` and `original` for its stale-edit check.

For `GoalState`, keep `knowledge`, `goal`, `revision`, `read`, `mutation`, `read_error`, `last_read`, and `direct_input` in the session; move `show`, `editor`, `editing`, `confirm`, and `scroll` to the view. Split `error`/`notice` by cause: backend request facts stay with the session, local draft/confirmation messages stay with the view. All `GoalRequest` fields belong to the session. All `GoalDraft` and `GoalConfirm` fields belong to the view.

For `SettingsState`, keep `future`, `revision`, `pending`, `deferred`, `active_before`, `errors`, and `legacy_overrides` in session facts. Keep all fields of the accepted `Change`, including its correlation token, desired values, turn/thread and requested persist/announce flags, in the session. Move `editor_error`, `rejected`, and not-yet-submitted `persist_next`/`announce_next` to the view; copy the latter two into the plain request at submit time. Disk persistence stays app-side, triggered once by the acknowledged result.

For `PanelState`, move `show`, `selected`, `scroll`, and `detail` to the view. Keep `agents`, `processes`, `snapshot`, `activity`, `stop`, `outgoing`, `error`, `control_error`, `notice`, `refreshed`, `older`, `revision`, `reports`, `older_window`, `ownership`, and `process_scopes` in the session. `older_window` describes which backend records Carlos has read, not a screen offset. Keep `VerifiedOwners`, `Process`, `Request`, `Snapshot`, `Activity`, and `Stop` entirely in the session. UI-specific wording/focus can use their plain facts; do not move their bounded read state into an overlay.

### Baseline host, commands, and reports


The author started from a clean Git working tree at `3f1916b2f7afd3b5e8291744923b329f2ef083af`. The host reported Linux `7.2.6-arch2-1`, x86_64 GNU/Linux, and `rustc 1.93.0 (254b59607 2026-01-19)`. The successful binary used the existing release profile (`lto = "thin"`, `codegen-units = 1`) with `--no-default-features`. That flag disables dictation. Other work advanced HEAD to `e46daddb0f70e178935fdcb9d62fc98ba5fbbd62` during authoring, but changed no Rust source, Cargo configuration, or benchmark fixture. The reports therefore still describe the final tree's executable source. Keep these compiler-specific reports when later recording the agreed Rust 1.100.0 baseline; do not compare builds from different compilers as if they differed only in this refactor.

The default-feature build failed with:

    Unable to find libclang: couldn't find any valid shared libraries matching libclang.so ...

The attempt using Whisper's existing bundled bindings flag then failed with:

    failed to execute command: No such file or directory (os error 2)
    is `cmake` not installed?

The author searched the repository and `benchmarks/` for `.jsonl` files, including ignored files outside build/dependency directories. Only `src/tests/fixtures/stale_transcript_replay.jsonl` existed. It is a fixed regression fixture, not a long personal session. The benchmark reads its `response_item` records only. Its small row count is expected.

The author ran this exact command three times, serially:

    rtk proxy ./target/release/carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48

First synthetic report:

    carlos perf-session
    source: synthetic seed=1 turns=1000 tool_lines=24
    viewport: 160x48
    transcript: messages=6001 rendered_lines=39347 relevant_items=6000 replay_elapsed_ms=53.38
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 15.91 p95 15.91 avg 15.91 max 15.91 ms
    full_layout:   16.75 ms
    full_draw:     4.16 ms
    scroll_draw:   p50 0.84 p95 1.07 avg 0.86 max 1.23 ms
    typing_draw:   p50 0.71 p95 0.79 avg 0.72 max 0.90 ms
    working_draw:  p50 0.71 p95 0.77 avg 0.72 max 0.94 ms
    append_total:  p50 0.70 p95 0.98 avg 0.72 max 0.98 ms
    layout_breakdown:
      tool_output_ansi msgs=888 lines=21312 total_ms=8.27
      assistant_markdown msgs=1000 lines=3618 total_ms=3.53
      user_plain msgs=1000 lines=3473 total_ms=1.48
      commentary_plain msgs=1000 lines=3407 total_ms=1.47
      diff msgs=112 lines=784 total_ms=0.37
      reasoning_markdown msgs=1000 lines=4500 total_ms=0.23
      tool_call_plain msgs=1000 lines=2250 total_ms=0.21

Second synthetic report (same source, viewport, counts, and zero-memory placeholders):

    replay_elapsed_ms=55.55
    replay_apply:  p50 16.51 p95 16.51 avg 16.51 max 16.51 ms
    full_layout:   18.57 ms
    full_draw:     4.34 ms
    scroll_draw:   p50 0.83 p95 0.99 avg 0.85 max 1.16 ms
    typing_draw:   p50 0.69 p95 0.79 avg 0.70 max 0.87 ms
    working_draw:  p50 0.69 p95 0.78 avg 0.70 max 0.88 ms
    append_total:  p50 0.71 p95 0.92 avg 0.73 max 0.92 ms
    layout_breakdown:
      tool_output_ansi msgs=888 lines=21312 total_ms=7.85
      assistant_markdown msgs=1000 lines=3618 total_ms=3.42
      user_plain msgs=1000 lines=3473 total_ms=1.42
      commentary_plain msgs=1000 lines=3407 total_ms=1.41
      diff msgs=112 lines=784 total_ms=0.36
      reasoning_markdown msgs=1000 lines=4500 total_ms=0.22
      tool_call_plain msgs=1000 lines=2250 total_ms=0.20

Third synthetic report (same source, viewport, counts, and zero-memory placeholders):

    replay_elapsed_ms=53.23
    replay_apply:  p50 15.72 p95 15.72 avg 15.72 max 15.72 ms
    full_layout:   17.59 ms
    full_draw:     4.05 ms
    scroll_draw:   p50 0.82 p95 0.93 avg 0.82 max 0.99 ms
    typing_draw:   p50 0.67 p95 0.71 avg 0.68 max 0.79 ms
    working_draw:  p50 0.67 p95 0.72 avg 0.68 max 1.02 ms
    append_total:  p50 0.69 p95 0.75 avg 0.70 max 0.75 ms
    layout_breakdown:
      tool_output_ansi msgs=888 lines=21312 total_ms=7.97
      assistant_markdown msgs=1000 lines=3618 total_ms=3.37
      user_plain msgs=1000 lines=3473 total_ms=1.42
      commentary_plain msgs=1000 lines=3407 total_ms=1.42
      diff msgs=112 lines=784 total_ms=0.35
      reasoning_markdown msgs=1000 lines=4500 total_ms=0.23
      tool_call_plain msgs=1000 lines=2250 total_ms=0.20

The author ran this exact command three times, serially:

    rtk proxy ./target/release/carlos perf-session src/tests/fixtures/stale_transcript_replay.jsonl --width 160 --height 48

First history-fixture report:

    carlos perf-session
    source: src/tests/fixtures/stale_transcript_replay.jsonl
    viewport: 160x48
    transcript: messages=5 rendered_lines=9 relevant_items=4 replay_elapsed_ms=1.33
    memory_kib: before=0 after_replay=0 after_bench=0
    replay_apply:  p50 0.01 p95 0.02 avg 0.01 max 0.02 ms
    full_layout:   0.00 ms
    full_draw:     0.64 ms
    scroll_draw:   p50 0.50 p95 0.50 avg 0.50 max 0.50 ms
    typing_draw:   p50 0.42 p95 0.49 avg 0.42 max 0.52 ms
    working_draw:  p50 0.42 p95 0.43 avg 0.42 max 0.52 ms
    append_total:  p50 0.45 p95 0.53 avg 0.46 max 0.53 ms
    layout_breakdown:
      assistant_markdown msgs=1 lines=2 total_ms=0.03
      reasoning_markdown msgs=1 lines=1 total_ms=0.01
      tool_call_plain msgs=1 lines=2 total_ms=0.00
      tool_output_ansi msgs=1 lines=1 total_ms=0.00

Second history-fixture report (same source, viewport, counts, and zero-memory placeholders):

    replay_elapsed_ms=1.15
    replay_apply:  p50 0.00 p95 0.02 avg 0.01 max 0.02 ms
    full_layout:   0.00 ms
    full_draw:     0.56 ms
    scroll_draw:   p50 0.79 p95 0.79 avg 0.79 max 0.79 ms
    typing_draw:   p50 0.42 p95 0.57 avg 0.44 max 0.64 ms
    working_draw:  p50 0.41 p95 0.43 avg 0.42 max 0.52 ms
    append_total:  p50 0.45 p95 0.54 avg 0.46 max 0.54 ms
    layout_breakdown:
      assistant_markdown msgs=1 lines=2 total_ms=0.02
      reasoning_markdown msgs=1 lines=1 total_ms=0.01
      tool_call_plain msgs=1 lines=2 total_ms=0.01
      tool_output_ansi msgs=1 lines=1 total_ms=0.00

Third history-fixture report (same source, viewport, counts, and zero-memory placeholders):

    replay_elapsed_ms=1.33
    replay_apply:  p50 0.00 p95 0.02 avg 0.01 max 0.02 ms
    full_layout:   0.00 ms
    full_draw:     0.63 ms
    scroll_draw:   p50 0.55 p95 0.55 avg 0.55 max 0.55 ms
    typing_draw:   p50 0.43 p95 0.49 avg 0.44 max 0.66 ms
    working_draw:  p50 0.42 p95 0.44 avg 0.42 max 0.53 ms
    append_total:  p50 0.45 p95 0.53 avg 0.46 max 0.53 ms
    layout_breakdown:
      assistant_markdown msgs=1 lines=2 total_ms=0.04
      reasoning_markdown msgs=1 lines=1 total_ms=0.01
      tool_call_plain msgs=1 lines=2 total_ms=0.01
      tool_output_ansi msgs=1 lines=1 total_ms=0.00

The reported message/line counts include the benchmark's final appended assistant entry and its 16 append steps. `relevant_items` counts the pre-benchmark replay. Do not mistake the extra message for a replay duplicate. `replay_elapsed_ms` includes replay, the profile pass, first layout, and first draw, rather than measuring JSON application alone. Use `replay_apply` for that separate cost. Memory remains unmeasured, as explained above.

### Binding baseline: Rust 1.100.0, default features (integrator, 2026-11-11)

The owner chose Rust 1.100.0. The checkout selects it with a local `rustup override` (pre-release build `0ef1bc386`, from `RUSTUP_DIST_SERVER=https://dev-static.rust-lang.org`). Tree `e46dadd`, `cargo build --release` with default features, then three serial runs of:

    ./target/release/carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48

Medians of the three runs (6,001 messages, 39,347 rendered lines):

    replay_elapsed_ms  49.27   (47.99, 49.27, 49.61)
    replay_apply       15.04
    full_layout        15.20   (15.01, 15.20, 15.82)
    full_draw           3.42
    scroll_draw  p50    0.71   p95 0.84
    typing_draw  p50    0.60   p95 0.63
    working_draw p50    0.60
    append_total p50    0.60

Compare later milestones with these figures, on the same machine and compiler. Treat a median that is more than 10 % slower as a regression that blocks the commit until explained. The Rust 1.93.0 reports above used `--no-default-features` and are kept for reference only.

## Interfaces and Dependencies


Use the existing standard library threads/channels and existing crates. Keep the session's external dependencies limited to the currently needed nonterminal crates: `anyhow`, `base64`, `blake3`, `libc`, `rusqlite`, `serde_json`, `shlex`, `tempfile`, `unicode_segmentation`, `unicode_width`, `uuid`, `jsonschema`, `url`, `mio`, `socket2`, and `tungstenite`. Admit `serde` only if needed without making the session depend on the dictation feature; this plan does not require new serialization derives. Move text helpers only if they are actually used by session facts; keep Markdown layout and syntax highlighting in the app. Do not add another runtime, dependency, or terminal adapter to the session.

The future-core local roots are `src/session/**`, `src/backend.rs`, `src/claude_backend/**`, `src/grok_backend/**`, `src/summaries/**`, `src/protocol*.rs`, `src/codex*.rs`, and `src/native_form.rs` with its declared children. Enumerate exact files in the guard and follow their module declarations. Include `src/protocol_socket/**` or other declared child directories if present when implementing the guard. App, theme, clipboard, dictation, event, CLI, rendering, and perf modules remain client-side. In plan 017, move these allowed roots together with their tests, then adjust crate exports/import roots and Cargo dependencies. Internal session files must already import `crate::session`, sibling session modules, or one of these terminal-free roots; none may import app bridges.

In `src/session/entry_id.rs`, provide an opaque type equivalent to:

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct EntryId { namespace: uuid::Uuid, counter: u64 }

Keep allocation private. In `src/session/models.rs`, `Message` gains `id: EntryId`; plain constructors that represent unadopted history use `EntryDraft` without an id. In `src/session/transcript.rs`, provide these operations or named equivalents with the same guarantees:

    pub fn entries(&self) -> &[Message];
    pub fn entry(&self, id: EntryId) -> Option<&Message>;
    pub fn entry_id_for_item(&self, key: &BackendEntryKey) -> Option<EntryId>;
    pub fn position(&self, id: EntryId) -> Option<usize>;
    pub fn append(&mut self, draft: EntryDraft) -> EntryId;
    pub fn replace_history(&mut self, drafts: Vec<EntryDraft>, same_thread: bool)
        -> SessionUpdate;
    pub fn truncate_before(&mut self, id: EntryId) -> SessionUpdate;

`position` serves only in-process rendering and store algorithms; a backend map, prompt reference, or delayed job must not cache its result. Keep mutating access private to the session tree. `replace_history` must receive drafts already checked for thread identity, limits, and parse errors; reconciliation and adoption must not leave a half-replaced vector. Prefer a typed replacement mode over `same_thread: bool` if it makes clear/reset/fork rules easier to review; it is a history policy type, not a backend event model.

In `src/session/state.rs`, expose plain operations in this shape, split across concern modules to respect file limits:

    pub struct SessionState { /* private grouped facts */ }
    pub struct SessionUpdate {
        pub dirty_from: Option<EntryId>,
        pub transcript_reset: bool,
        /* plain backend actions and explicit editor recovery outcomes as needed */
    }

    impl SessionState {
        pub fn new(thread_id: String, options: SessionOptions) -> Self;
        pub fn entries(&self) -> &[Message];
        pub fn handle_backend_line(&mut self, line: &str) -> SessionUpdate;
        pub fn submit_text(&mut self, client: &dyn BackendClient, text: String,
            origin: PromptOrigin, priority: PromptPriority) -> SubmitOutcome;
        pub fn interrupt(&mut self, client: &dyn BackendClient) -> SessionUpdate;
        pub fn tick(&mut self, client: &dyn BackendClient, now: Instant) -> SessionUpdate;
        pub fn next_deadline(&self, now: Instant) -> Option<Duration>;
    }

Name plain request/result types for rewind, approvals, editor recovery, and settings where needed. Do not use these types as a shadow event stream. A synchronous caller consumes each change report once, applies existing backend actions in order, and invalidates its view. `SessionOptions` carries explicit cwd, Ralph settings, backend settings, and the tool-label preference loaded by app startup. A session constructor must not load UI preferences or create a `TextArea` indirectly.

In `src/app/state.rs` and `src/app/view_state.rs`, finish with:

    pub(super) struct AppState {
        pub(super) session: SessionState,
        pub(super) view: ViewState,
    }

    pub(super) struct ViewState {
        /* composer, navigation, render caches, overlays, local drafts and dictation */
    }

In `src/claude_backend/event_normalizer.rs`, provide one backend-owned state machine that accepts accepted controller messages and emits ordered common JSON lines plus named backend facts. Separate live and history translator cursors. Route every emitted line through `EventSender` so generation stamping, frame/byte bounds, and acknowledgements still work. Do not expose the translator to the app or session. Test this owner at the `BackendClient` receiver boundary, rather than only unit-testing `translate_claude_line`.

Plan-author note, 2026-11-11: this initial version records verified field owners, exact history/id rules, buildable work slices, the future-core file boundary, and measured reports. It leaves implementation and default-feature preflight unchecked because this task permits only the planning file and release/perf commands. Later edits must add a dated note explaining what changed and why.

Plan-author review note, 2026-11-11: the author checked the initial draft against the source again, clarified Claude's session-adoption point so the plan preserves startup-failure behavior, made the parallel workers' test scopes disjoint, and made the guard list external core tests explicitly. The author also recorded the normalizer's lock/order rules to prevent deadlocks while moving work off the UI path and named the tests that must prove the id rules.

Plan-author source-check note, 2026-11-11: other work changed the architecture document during authoring. The author verified the complete intervening file list, kept the unchanged-source benchmark valid, and recorded the future shared-client and compiler decisions without adding either task to 016.

Handover note, 2026-11-12: recorded the recovered source, remaining work, and the owner’s instruction to continue directly without agents.

Milestone 7.3 note, 2026-11-12: recorded the callback/browser moves, exact comparison and the no-copy correction.

Milestone 8.1 note, 2026-11-12: moved queue facts and the shared native token counter, preserved local editor/recovery ownership, and recorded checks. Seven quiet alternating speed pairs: {"synthetic": [{"replay": 47.61, "layout": 14.67, "draw": 3.53, "scroll": 0.7, "typing": 0.6}, {"replay": 47.7, "layout": 14.63, "draw": 3.47, "scroll": 0.7, "typing": 0.59}], "fixture": [{"replay": 1.22, "layout": 0.01, "draw": 0.49, "scroll": 0.44, "typing": 0.37}, {"replay": 1.05, "layout": 0.0, "draw": 0.48, "scroll": 0.43, "typing": 0.36}]}. The earlier run overlapped event replay and is not used as the quiet speed check.
