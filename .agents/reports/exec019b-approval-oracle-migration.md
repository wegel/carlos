# EP019b frozen approval replies

The Core tests keep both JSON files captured from commit `2cbfedc` unchanged. They no longer paint the removed embedded overlay. The core receives the original request inputs and builds answers with the same typed converters used by actual daemon controls.

## Frozen bytes

- `crates/carlos-core/src/session/fixtures/m4_approval_lifetimes.json`: 646951 bytes, SHA-256 `865ed204aa7b14346089218453120727e29257ce4875f08a15d2fbd5266216be`.
- `crates/carlos-core/src/session/fixtures/m4_view_expected.json`: 145540 bytes, SHA-256 `fbec966010fefa822d1d50669ba549c67d4d05966bd37e831594b3e2101867dc`.

No test writes these files. Git retains the original blobs when moving them.

## Native reply checks

`approval_frozen_oracle::all_frozen_lifetime_native_answers_keep_their_original_envelopes` visits all 470 cases and their 2406 steps. For each of 1875 recorded cumulative native answers, it checks the original native id and complete body against replies built from that case's original backend/method or Claude payload kind. It retains all 44 distinct exact id/body tuples. The independent input builder offers typed decisions, original choice values, omitted fields, free text, integers 2 and 22 and the original changed-command input. It never derives an answer from the frozen reply body.

`approval_frozen_oracle::all_frozen_view_native_answers_keep_their_original_envelopes` visits all 273 cases and checks all 204 recorded native answers, including all 12 distinct exact id/body tuples. Each synthetic kind uses the original `ApprovalRequestDraft` input and the production reply builder. Actual native cases use the original callback method/payload and typed converter. A reply from another request kind cannot satisfy that case. Native id `request` remains exact, including the original synthetic permissions payload.

The lifetime corpus also records 36 cumulative ordinary-control calls, with three distinct tuples. Their exact retained tuple set remains asserted. Guarded controls have separate successors: `control_work_tests::a_changed_turn_or_dropped_owner_cancels_a_prepared_interrupt`, `test_codex_reconnect_keeps_drafts_uses_current_fork_and_replays_full_items_without_duplicates`, and `test_claude_native_reset_preserves_active_form_two_queued_callbacks_and_early_new_callback`. Those tests exercise actual commands; the reply projection does not treat a native callback answer as an ordinary chat command.

## Card lifetime and private editor checks

The JSON files also contain old whole-cell hashes, keyboard detail lines, local confirmation and the old global focus/queue shape. Those clauses are archived in their unchanged files, rather than compared with the daemon's independently selected Device panels. The current checks divide those responsibilities:

- `approval_operation_sequences::twenty_thousand_public_approval_sequences_preserve_identity_and_claims` retains four fixed seeds, 20,000 cases, 32 steps, all 28 operation classes and five issued-id classes. It checks exact guarded native writes, immutable per-id facts, issued/retired ids, one claim and unknown/foreign ids. The original 200,000 case/128 step run remains ignored and available.
- `session_update_tests::{queued_and_retired_copies_receive_new_addresses_when_adopted,retired_native_request_cannot_return_through_either_queue_or_answer_a_reused_id}` retain copied-facts and retired-id checks in the core.
- Actual Device `questions_ownership_tests::{first_consent_and_local_hints_leave_every_accepted_shared_fact_unchanged,new_opaque_card_cannot_inherit_old_consent_while_duplicate_metadata_keeps_it}` check private first consent, replacement and unchanged metadata across all three backends.
- `questions_tests::{two_devices_cycle_focus_without_changing_shared_active_cards_or_other_drafts,unknown_resolution_keeps_both_actual_device_drafts_and_explicit_focus,readonly_caller_and_browser_progress_details_keep_private_confirmation_and_answer}` retain independent focus, queued drafts and metadata-only refresh.
- Actual daemon/PTY tests `test_daemon_f4_two_private_question_drafts_keep_native_ids_and_older_task_selection`, `test_daemon_retired_question_draft_never_answers_a_replacement_native_card`, `test_codex_active_and_queued_resolutions_preserve_phone_drafts_without_replay` and `test_claude_native_reset_preserves_active_form_two_queued_callbacks_and_early_new_callback` assert visible panels, private drafts and original native RPC targets.

The separate view report records the 273 case painter/detail map. The nine fixed lifetime tests remain registered until their own native/private clauses are retired or moved; this checkpoint removes only the 470 case embedded comparator.

## Checks

- Both Core oracle tests pass after moving the JSON files: `/tmp/carlos-approval-frozen-moved.log`.
- Strict no-default, all-target workspace Clippy passes: `/tmp/carlos-approval-frozen-clippy-final.log`.
- Full workspace formatting and the Rust400/60 size checks pass.
- The normal commit hook runs the full workspace Rust tests. Its log is `/tmp/carlos-approval-frozen-commit.log`.

This checkpoint changes tests and their fixture locations. It changes no native protocol, terminal keys or product runtime.
