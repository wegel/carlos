# Callback tests at their actual owners

Twenty old tests mixed native request facts, editor input and backend writes through AppState. Their successors now call the Core request owner, the existing pure editor or the actual Device. None of these terminal successors constructs SessionState or BackendClient.

## Native replies and editor input

The five old `question_split_answers.rs` cases retain these checks:

- `claude_tool_and_browser_answers_keep_the_exact_native_envelopes` now lives in Core `approval_answer_contract_tests`. The original tool, plan and URL responses keep the original callback ID and exact body at the guarded writer.
- `claude_single_multiple_and_other_answers_preserve_input_and_whitespace` is split between that Core module's native-envelope test and `questions::answer_tests::claude_single_multiple_and_other_fields_preserve_selected_values_and_whitespace`. Original input fields, multiple values, Other and whitespace survive.
- `every_mcp_field_kind_keeps_its_typed_value_and_default_in_both_backends` is split between `both_backends_return_every_typed_form_value_without_inventing_optional_fields`, `deferred_codex_form_checks_keep_every_original_schema_value_and_revision` and the pure editor's corresponding default test. All eight original types, order, defaults, optional omission and complete native bodies remain. Each deferred check asserts the exact original schema, content, ID and revision, then the exact checked reply. The two legacy schema spellings needed the separately committed repair in `fae0288`; its causal helper and terminal proofs remain in the form-compatibility report.
- `codex_free_text_choice_notes_other_and_omission_keep_exact_answers` now checks raw editor values in `questions::answer_tests` and the normalized native `user_note` body in Core. The owner retains the original native question key.
- `both_backends_keep_parser_errors_and_permissive_defaults` now calls `NativeDraft::from_input` with neutral typed fields in `typed_form_descriptions_keep_original_parser_errors_and_empty_choices_allow_text`. Native envelope adapters do not enter this terminal test.

## Request lifetimes and private drafts

The five old `question_split_lifetimes.rs` cases retain separate native and private checks:

- The two cancellation cases now use Core `cancelling_either_numeric_or_string_callback_never_retires_the_other` and actual Device `retiring_either_actual_device_card_keeps_the_other_answer_and_explicit_focus`. Numeric `9` and string `"9"` stay separate native requests. Either survivor keeps its exact private draft and explicit local focus.
- `late_form_checks_cannot_send_after_candidate_revision_or_lifetime_changes` now runs directly in Core `callback_boundary_tests`, retaining all four original stale-result causes and the no-write result.
- `callbacks_arriving_after_root_completion_still_receive_independent_editors` now uses Core `a_child_question_after_root_completion_keeps_its_own_native_request_and_fields`, the actual Device lifetime tests and the unchanged actual terminal late-callback workflow. The Core test leaves the completed root turn unset and retains the child's own fields and request ID.
- `native_reset_keeps_the_unresolved_process_question_and_its_draft` now uses existing Core `claude_clear_keeps_process_card_ids_and_duplicate_clear_retires_rows_once` and `native_clear_keeps_callback_identity_until_its_exact_cancellation`, plus actual Device `native_reset_keeps_three_exact_private_drafts_and_confirmation_until_each_card_retires`. The stronger Device case keeps three private drafts and their consent, then retires only the addressed cards. No snapshot progress sends an answer or chat.

The Device tests also prove that a new terminal cannot inherit the old composer, draft or consent, and that a blocking child gets local focus while Core still retains the original nonblocking root as its shared active card. F4 does not mutate shared focus. A fifth Device case explicitly instantiates Browser input: its waiting consent draft survives reset and disappears only when that exact native card is cancelled; the other draft and main composer survive.

## Bounds, worker order and notification facts

The four old `codex_callback_split_tests.rs` cases map as follows:

- The original 14 typed ID/boundary inputs and the 2,048-ID replay window now run unchanged in Core `callback_boundary_tests`.
- The original schema-preflight ordering now runs in `schema_preflight_does_not_consume_the_pending_candidate`. Revision zero can update the checked error without consuming the pending candidate; revision one returns the original complete reply. Existing Device `late_preflight_error_keeps_the_exact_schema_draft_and_confirmation` and the unchanged failed-reply terminal workflow retain the private error and draft checks.
- The clear/reset case uses the native clear identity tests and the actual Device reset test named above. The existing actual terminal `test_claude_native_reset_keeps_pipelined_human_once_and_uses_observed_session_id` preserves `/clear` as a native prompt; `test_claude_native_reset_preserves_active_form_two_queued_callbacks_and_early_new_callback` preserves the callback order, exact form value and one-prompt count across a native reset.

All six cases formerly included from `notification_tests/context_and_approvals.rs` now live under Core `common_native_tests::callback_notifications`, with their original names, native JSON strings and expected reply bodies. They retain command details and offered controls, turn/session permission grants, the synthetic Claude plan's object ID and plan/allowed prompts, accept/decline/session restrictions, the unsupported-method error and the pending plan after root completion. The new seventh case covers a later child question. The Core helper feeds the real backend reducer; no new owner facade exists.

## Evidence and limits

Core boundary five, native-answer five and notification seven tests pass in `/tmp/carlos-callback-{boundary-final,answer-final,notifications-final}.log`. The four pure editor and five actual Device tests also pass in `/tmp/carlos-callback-device-editor-final.log`. The immutable native reply corpora and fixed-seed owner model remain separately tracked and unchanged.

The combined primary source passes the no-default-feature workspace tests: 1,558 pass and nine remain ignored. Strict all-target Clippy, the format check and the 400/60 style check pass in `/tmp/carlos-callback-owners-{workspace,clippy,fmt,size}.log`. The compiler identified four now-unused test forwards/reexports, which are removed with their last callers. These checks include the sibling's coherent history/visibility test move; this report does not claim an isolated source tree for that sibling work.

Five unchanged actual daemon terminal workflows pass in 3.042 seconds: Codex concurrent child questions and standard typed forms; Claude concurrent child requests, late failed reply and native reset with queued callbacks. The tests assert both the visible screen and exact native requests. Log: `/tmp/carlos-callback-boundary-terminal.log`; artifacts: `/tmp/carlos-ep019b/callback-boundary-terminal`. They use the frozen matching pair `9a66816a2ffa2e849eabe2479c1c73be` described in the process-selection report.

The parent Codex question cases, its parser case and the old approval-lifetime family remain registered until their remaining clauses have explicit successors. One parent case exposed a separate Esc gap: a displayed nonblocking draft can hide a queued blocking card. Its causal actual-terminal failure is `/tmp/carlos-question-escape-before.log`; its repair belongs to the next bounded checkpoint and is not claimed by this green test move. This checkpoint changes tests and test support only. It neither deletes the production embedded path nor marks EP019b complete. No reviewer or install ran.
