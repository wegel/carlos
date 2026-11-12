# Carlos milestone 5a report

This checkout is `/var/home/wegel/work/perso/carlos-daemon`, branch `daemon-split`. HEAD stayed at `2ec8d5bca8b19614e4caff1bcc88974aedfe3c74`. I changed part 5a only. I did not touch the owner's checkout, `.agents/`, or `docs/`; commit, push, stash, reset, install, and toolchain changes were not run. Builds used the existing Rust 1.100.0 override. Temporary source copies used the same already installed toolchain through `RUSTUP_TOOLCHAIN=1.100.0`.

The session now owns backend capabilities, current and pending model/effort/summary choices, and the model catalog. The app reads those facts through borrowed accessors and writes them through session methods. `ViewState.runtime_settings` owns every editor field and the overlay's visibility. The app still reads and writes preferences, edits settings, and sends backend controls. The existing files keep their paths and formats.

## Per-file changes

- `src/app/backend_setup_grok.rs`: Pass loaded startup runtime choices into the app/session constructor.
- `src/app/backend_setup_launch.rs`: Pass loaded choices into Codex/Claude constructors and read inherited choices through session accessors.
- `src/app/claude_context.rs`: Read the current model through the session accessor.
- `src/app/claude_models.rs`: Keep Claude guards and decisions in place; use session methods for model/effort writes and app methods for editor reads.
- `src/app/claude_session_history.rs`: Keep inherited-choice guards during session switches and send the same clears through session methods.
- `src/app/codex_reconnect_install.rs`: Read settings-overlay visibility from the view while preserving reconnect draft capture.
- `src/app/codex_settings.rs`: Read session choices and delegate native field acknowledgements/pending clears to session methods.
- `src/app/codex_settings_draft.rs`: Keep validation, rejected editor drafts, persistence, and startup order in the app; route fact writes through session methods.
- `src/app/input_events_keys.rs`: Read overlay visibility from the view; split existing key-count and modal-key code to keep functions below 60 lines.
- `src/app/input_events_model_settings.rs`: Edit view fields, snapshot pending choices through the session, and keep default-file writes in a small app helper.
- `src/app/mod.rs`: Replace the local RuntimeDefaults declaration with its session type import.
- `src/app/mouse_events.rs`: Read visibility/draft fields from the view for mouse and paste handling; keep the handler within 60 lines.
- `src/app/overlay_render_model_settings.rs`: Draw editor fields from the view while borrowing capability/model facts from the session.
- `src/app/render_sections.rs`: Read settings-overlay visibility from the view at the existing render branches.
- `src/app/runtime_settings_model_settings.rs`: Keep editor field lists, draft choices, fallbacks, and effort refresh in the view, borrowing session facts.
- `src/app/runtime_settings_state.rs`: Keep only overlay/editor state and its unchanged defaults, navigation, and draft edits.
- `src/app/state.rs`: Remove the mixed runtime child and pass explicit RuntimeDefaults into SessionState through the app constructor.
- `src/app/state_settings.rs`: Delegate fact methods to the session and editor methods to the view; preserve apply/open/close order and hints.
- `src/app/view_state.rs`: Own and construct RuntimeSettingsView.
- `src/session/mod.rs`: Register runtime settings and native-update modules.
- `src/session/runtime_settings.rs`: Own RuntimeDefaults, capabilities, current/pending choices, catalog, read accessors, and the moved settings methods.
- `src/session/runtime_settings_updates.rs`: Own exact native writes, inheritance clears, pending override drops, acknowledgements, and rejected-choice matching.
- `src/session/session_update_tests.rs`: Supply explicit empty startup choices to existing session test constructors.
- `src/session/state.rs`: Own RuntimeSettingsState and accept explicit startup choices without reading app files.
- `src/session/turn_boundary_tests.rs`: Supply explicit empty startup choices to the existing constructor.
- `src/tests/claude_context_tests.rs`: Replace direct model test setup with the exact session write method.
- `src/tests/claude_sdk_tests.rs`: Keep all expected values and borrow moved facts once per assertion group to stay below 400 lines.
- `src/tests/claude_session_tests.rs`: Route exact model/effort test setup through session methods and read the moved facts.
- `src/tests/claude_visibility_tests.rs`: Read overlay visibility from the view in the existing assertion.
- `src/tests/codex_reconnect_identity_tests.rs`: Read editor fields from the view and acknowledged choices from the session without changing expected results.
- `src/tests/codex_settings_tests.rs`: Read model/catalog facts through accessors and edit/assert view fields at the existing test steps.
- `src/tests/codex_startup_settings_tests.rs`: Keep constructor/startup expectations and editor edits through their new owners.
- `src/tests/input_tests/dictation.rs`: Read settings-overlay visibility from the view in the existing assertion.
- `src/tests/runtime_tests.rs`: Include the ten new settings parity tests.
- `src/tests/runtime_tests/ralph_runtime.rs`: Move existing settings-editor test field access to the view; leave Ralph tests unchanged.
- `src/tests/runtime_tests/runtime_settings_parity.rs`: Add ten deterministic tests with explicit expected cases, run against HEAD and the changed source.
- `src/tests/tool_output_tests.rs`: Move settings-editor assertions to view fields without changing tool-output behavior or expected values.

## Method parity, listed for narrow screens

Each item names the old method or write, the guards/defaults/order that remain, and its current home.

- `RuntimeSettingsState::new`: capabilities start true, current/pending values start absent, and the catalog starts empty for `AppState::new`; editor defaults stay Model/Auto/empty text/index 0, effort medium/index 3, summary auto/index 0. Now split between session `runtime_settings.rs::RuntimeSettingsState::new` and app `runtime_settings_state.rs::RuntimeSettingsView::new`.
- Startup: app `with_runtime_defaults` supplies plain `RuntimeDefaults` to `SessionState::new`. Constructor inputs use the old startup setter's trim-and-drop-empty rule. The three backend launch functions pass their already loaded preferences. The app still resolves catalog/native/default choices with the same precedence, calls its later setters, and handles startup errors. Backend events still wait until that app setup finishes. Temporary apps call `AppState::new` with empty choices.
- `set_capabilities`: assign both flags first; clear current and pending effort when effort is unsupported, then clear both summary fields when summary is unsupported; leave model/catalog alone. Now session `set_runtime_capabilities`.
- `set_runtime_settings`: trim all supplied strings; empty or absent input clears each corresponding current value; do not check capability flags or change pending fields. Now session method of the same name.
- `merge_runtime_settings`: model first, then effort only when supported, then summary only when supported; ignore empty/absent inputs rather than clear current values. Now session method of the same name.
- `set_available_models`: stable default-first sort, then replace the catalog; no editor refresh or deduplication added. Now session method of the same name.
- `queue_runtime_settings`: trim and replace all three pending fields, including clearing absent inputs; do not check capability flags. Now session method of the same name.
- `take_pending_runtime_settings`: despite its old name, clone all three values without consuming them. Now session `pending_runtime_settings(&self)`; the test-only app wrapper retains its previous name. The settings save handler uses the same three small clones.
- `next_turn_runtime_settings`: independently prefer each pending value over its current value, in model/effort/summary order. Now session method of the same name.
- `mark_runtime_settings_applied`: do nothing when every pending field is absent; otherwise copy ALL three pending fields into current values and then clear ALL pending fields. This deliberately retains the previous loss of unspecified current values when only one override is present. Now session method of the same name.
- `runtime_settings_label` / `runtime_settings_pending`: prefer pending fields independently; default model to `model?`; join present values with `/`; append `*` only when at least one pending value exists and any pending/current pair differs, including absent pending values. Now session methods of the same names.
- `open_model_settings`: copy tool-output/theme/label drafts first; show overlay and select Model; prefer pending model, then current, then empty; choose matching catalog index or index 0, catalog text before free text; refresh effort; select pending/current summary or auto, falling back to index 0; then apply Codex editor refresh and rejected-choice restore. Now app `state_settings.rs` plus `RuntimeSettingsView::open_model_settings`, borrowing the session's facts.
- `close_model_settings` / `toggle_model_settings`: close changes visibility only; toggle follows the same open/close paths. Now app view and app wrapper.
- `model_settings_fields`: Model first; Effort only when supported and editor options are nonempty; Summary only when supported; then DictationEndpoint, ToolLabels, DesktopTheme, PhoneTheme, ToolOutput. Now app view method borrowing capabilities.
- `model_settings_move_field`: retain the `len <= 1` fallback to Model, missing-current fallback to 0, and forward/backward wrap. Now app view method borrowing capabilities.
- `model_settings_cycle_effort`: refill empty choices with the six existing defaults; signed wrap with `rem_euclid`; do not write session choices. Now app view.
- `model_settings_cycle_model`: return immediately on an empty catalog; signed wrap; copy selected model text; then refresh effort choices. Now app view borrowing the catalog.
- `model_settings_cycle_summary`: preserve signed wrap and the existing assumption that editor summary choices are nonempty. No new guard or guarantee. Now app view.
- `model_settings_has_model_choices`: check only whether the catalog is empty. Now app view borrowing the catalog.
- `model_settings_insert_char` / `model_settings_backspace`: push/pop one Unicode character in free-text draft only. Now app view.
- `model_settings_model_value`: catalog entry at the selected index wins; otherwise use free-text draft. Now app view, returning a borrow with the shared view/facts lifetime.
- `model_settings_effort_value`: selected editor option or `medium`. Now app view.
- `model_settings_summary_value`: return empty text when unsupported; otherwise selected editor option or `auto`. Now app view borrowing capabilities.
- `refresh_model_settings_efforts`: pending effort, then current, then medium; selected model's nonempty options or existing defaults; match requested value ignoring ASCII case, then advertised default ignoring ASCII case, then default-medium index 3 bounded to the options' last index. Now app view; the preexisting copy of the small editor option list remains.
- `apply_model_settings`: apply AI-label draft first, then tool-output and theme drafts; normalize editor choices and omit unsupported effort/summary; queue the three choices; close the overlay; then choose the same supported-summary label and active-turn hint. Now split between app `state_settings.rs`, view `apply_model_settings`, and session `queue_runtime_settings`. No disk I/O moved.
- Test-only `apply_default_reasoning_summary`: write normalized pending summary only when both current and pending summary are absent. Now session method through the unchanged app test wrapper.
- `normalize_non_empty`: trim, drop empty, allocate the trimmed string. Now one session helper for session writes and one app helper for editor choices. `default_effort_vec` and `effort_index` stay app-side with exactly their existing defaults and case rules.
- Codex `apply_future`: inspect model, effort, summary in that order; skip absent JSON fields; supplied null/non-string clears current, strings stay untrimmed; clear pending only on exact equality with the supplied string. Now session `apply_future_runtime_settings`; the app retains its wrapper and caller order.
- Codex `observed`: keep thread-id/object guards and revision/future/deferred ordering; when no deferred request exists clear all pending values before applying native fields. The app now calls the existing session queue method with three absent values, then the moved acknowledgement method.
- Codex `initialize`: keep resumed/new choice rules, explicit summary override precedence, parse-before-mutate, native setter, pending clear, future capture, equal-field removal, raw pending writes, deferred request assignment. Raw pending writes now use session `replace_pending_runtime_choices`; they still retain empty/spaced strings.
- Codex `reject`: keep safe error text and the deferred-request guard for editor errors; then remove only pending values that still exactly match the rejected request, in model/effort/summary order. The app keeps editor state; session `reject_runtime_settings` owns the three pending writes.
- Other Claude/Codex direct writes: `record_runtime_choice` preserves exact current values without trimming or touching pending values; `clear_pending_runtime_choice` drops only its named pending field; `inherit_runtime_effort` clears current and pending effort. Callers retain their existing closed-phase, capability, inherited-choice, native-observation, and stale-value guards. No backend handler moved to a new thread.
- Keyboard helper split: `record_key_event` still counts every key kind, returns before counting a non-press as a key event, and counts presses before routing controls. `handle_native_question_key` keeps draft-first handling inside the same modal guard, then handles approval keys. This small split keeps the edited existing function within 60 lines.
- `save_settings` helper split: preserve disconnected/validation returns, snapshot-before-apply, native save/live apply, summary preference save, appearance save, then runtime-default save. `persist_settings_defaults` keeps the existing Codex skip and the same failure hint. Both remain app-side.

## Tests added and reference checks

The ten tests in `src/tests/runtime_tests/runtime_settings_parity.rs` enumerate expected values explicitly:

- Empty/absent/spaced inputs for set, merge, and queue; repeated pending reads prove they do not consume values.
- All four effort/summary capability pairs, clearing behavior, merge guards, and the intentionally unguarded setter/queue behavior.
- Six partial/equal/absent pending triples, exact labels, next-turn fallbacks, and all-fields-on-apply behavior.
- Six default-summary cases covering existing current/pending values and empty input.
- Stable order of two default and two nondefault models; pending choice precedence and case-insensitive effort matching.
- Five effort fallback cases, including empty advertised options and a one-item catalog choice list.
- Free-text Unicode edits, empty-catalog cycling, reverse effort/summary cycling, and cancel without changed session facts.
- Five native field cases (absent, null, non-string, empty, spaced) and three rejected/stale override cases.
- Three resumed-start summary overrides proving raw empty/spaced values survive.
- Three explicit constructor inputs, compared with the old startup setter's behavior.

All ten passed on both the changed source and a temporary copy of HEAD. Both ran the same tests with the same literal expectations. The HEAD probe supplied only a test adapter for the new app constructor, implemented as old `AppState::new` followed by old `set_runtime_settings`. Test logs: `/tmp/carlos-m5a/{before,after}-enumerated-tests.log`. Existing tests keep every assertion; only field paths, session setters, and short borrowed aliases changed.

## Live-event comparison

- Reference: HEAD `2ec8d5bca8b19614e4caff1bcc88974aedfe3c74`, built from a temporary archive of this checkout's commit. No other checkout was used.
- Sample: Python `random.Random(516005).sample(corpus, 3000)` from the saved 24,300-stream corpus at `target/m4-fix/streams.jsonl`. The sample contains 1,040 Codex, 986 Claude, and 974 Grok streams; 343 include history loads.
- Result: 3,000 complete streams, 54,496 event checkpoints, zero panics on either side, zero differences. The harness compared session facts (extended to include all moved runtime facts/catalog), ordered backend calls, the ordered redraw log, and rendered lines/styles at 160x48 and 60x20. Both output files have SHA-256 `26b18a7b2dfb5738dfcdd89a04937172e81c081ae67a1d749d476579cac1ee76`.
- Reproduce/evidence: `/tmp/carlos-m5a/prepare.py`, `compare.py`, `streams.jsonl`, `{before,after}-audit.rs`, `{before,after}-audit-bin`, and `comparison.json`. The harness adds hooks only to temporary copies. All 37 changed source files match the source used by the successful comparison (`snapshot-check.json`, `source-hashes.json`).

The saved harness's input actions include live backend events, history loads, submit/interrupt/approval/rewind, callbacks, and a small set of keys. It does not exercise settings-editor Enter/Escape/navigation, actual disk writes, or backend startup with nonempty saved preferences. The ten old/new tests above cover editor and constructor cases; existing runtime preference tests and the required terminal suite cover disk formats and complete controls. I did not claim that the stream sample exercises those paths.

## Required checks

- `cargo build`: PASS; 8.11 s.
- `cargo build --release`: PASS; 46.56 s.
- `cargo fmt --check`: PASS.
- `bash scripts/check-rust-style.sh --all`: PASS.
- First accepted `cargo test`: PASS; 874 passed, 3 ignored, 3.76 s.
- Next `cargo test`: only the documented `protocol::history_transport_tests::unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls` failed (873 passed, 1 failed, 3 ignored, 4.29 s). It hit the existing line 111 check with `Codex stdin deadline expired; request may already have been accepted`.
- Required retry: PASS; 874 passed, 3 ignored, 3.73 s. Both required successful suites stay below 8 s. Logs and all attempts are in `/tmp/carlos-m5a/tests.json` and `test-{1,2,3}.log`.
- `bash scripts/test.sh`: PASS on its single invocation; its Rust suite passed with 874 tests and 3 ignored in 5.17 s, and all 174 terminal workflows passed in 77.747 s. It also passed the controller, recorder, and package checks. Screens/requests are under `target/ui-artifacts/run-Iq95AP/`; full output is `/tmp/carlos-m5a/terminal.log`.
- All compiler runs report only the existing `protocol_writer.rs` atomic-method deprecation. No new compiler warning.
- `git diff --check`: PASS; branch is still `daemon-split`; `.agents/` and `docs/` have no changes. No Rust toolchain file or override changed (`rustc 1.100.0 (0ef1bc386 2026-11-09)`).

I discarded two preliminary test runs because Cargo reused the instrumented temporary-copy executable from the shared target directory (4 ignored tests). I forced a rebuild of this checkout's unchanged source before the accepted runs above; those logs show the expected 3 ignored tests and no temporary harness test. The saved sample executables and source hashes remain valid. These discarded runs are saved as `probe-cache-test-{1,2}.log`.

The changed files have at most 400 lines and their functions have at most 60 lines, checked with the earlier milestone's lexical function scanner (`/tmp/carlos-m5a/function-sizes.json`). The new session files import no app or terminal code. No dependency or renderer algorithm changed. Accessors borrow the catalog; no new frame/row/delta traversal or transcript-sized copy was added.

## Files outside Milestone 5's named list

- `src/app/claude_context.rs`: Read the current model through the session accessor.
- `src/app/claude_models.rs`: Keep Claude guards and decisions in place; use session methods for model/effort writes and app methods for editor reads.
- `src/app/claude_session_history.rs`: Keep inherited-choice guards during session switches and send the same clears through session methods.
- `src/app/codex_reconnect_install.rs`: Read settings-overlay visibility from the view while preserving reconnect draft capture.
- `src/app/codex_settings.rs`: Read session choices and delegate native field acknowledgements/pending clears to session methods.
- `src/app/codex_settings_draft.rs`: Keep validation, rejected editor drafts, persistence, and startup order in the app; route fact writes through session methods.
- `src/app/input_events_keys.rs`: Read overlay visibility from the view; split existing key-count and modal-key code to keep functions below 60 lines.
- `src/app/mouse_events.rs`: Read visibility/draft fields from the view for mouse and paste handling; keep the handler within 60 lines.
- `src/app/render_sections.rs`: Read settings-overlay visibility from the view at the existing render branches.
- `src/tests/claude_context_tests.rs`: Replace direct model test setup with the exact session write method.
- `src/tests/claude_sdk_tests.rs`: Keep all expected values and borrow moved facts once per assertion group to stay below 400 lines.
- `src/tests/claude_session_tests.rs`: Route exact model/effort test setup through session methods and read the moved facts.
- `src/tests/claude_visibility_tests.rs`: Read overlay visibility from the view in the existing assertion.
- `src/tests/codex_reconnect_identity_tests.rs`: Read editor fields from the view and acknowledged choices from the session without changing expected results.
- `src/tests/input_tests/dictation.rs`: Read settings-overlay visibility from the view in the existing assertion.

These edits remove references to fields whose owner changed. The keyboard file also needed the small helper split above to satisfy the 60-line cap; the mouse handler lost two blank lines to meet that cap. Claude test assertions borrow settings once per assertion group so their existing file stays below 400 lines. No unrelated backend decisions or protocols changed.

## Discoveries and decisions for the plan

- The old `take_pending_runtime_settings` did not take anything. The session method now says `pending_runtime_settings`; the old app test wrapper still proves the original behavior.
- The old apply method overwrites all current fields when any pending field exists. A future daemon may want per-field updates, but this part preserves the old behavior and adds a test for it.
- The old set and queue methods accept effort/summary even after capabilities disable them; merge alone checks those flags. A future daemon may want to reject unsupported choices, but this part adds no such guarantee.
- Codex keeps raw strings from native acknowledgements and launch overrides. Ordinary setters and editor saves trim strings. The session keeps those separate methods so it does not silently strengthen native checks.
- The editor alone owns its model/effort/summary option indices and draft choices. The session retains one model catalog; the view borrows it, copying only the same small editor option lists that the old code copied.
- The app still loads startup files and resolves backend-specific defaults. It passes plain saved choices to the session constructor; existing backend setup still resolves and installs the final native/catalog/default result in its previous order.
- Parts 5b, 5c, and 5d remain untouched: input history, tool-label summaries/classification helpers, and rate limits/quota arithmetic retain their current owners.
- The known wall-clock Codex transport test remains unchanged. No additional guarantee about its 20 ms deadline was introduced.

Nothing requested for part 5a remains unfinished. The sample is a finite test, not an exhaustive proof; the report names its uncovered paths above. The existing Codex timing fault and the remaining milestone parts remain outside this task. I did not rerun transcript speed benchmarks because this part changes no transcript traversal, parser, redraw algorithm, or renderer algorithm; the comparison still checks the exact redraw log and rendered output.
