# Final local phone, process and quota tests

The terminal tests below now use the real Device or plain display data. The
backend worker owns the remaining 22 native edit cases and 11 process cases;
this slice does not duplicate their native calls, UUID matching or phase guards.

## Five local counterparts

- Old `claude_edit_tests::phone_draws_progress_and_failure_even_when_an_older_rewind_row_is_selected`
  keeps its name under `remote/history_progress_tests.rs`. It draws the real
  Device at 36×12 and then 18×12, with the first prompt selected above 80 retained
  answer rows. It checks the original `Preparing edited branch`/`Editing` and
  `Edit failed` text while the viewport remains at the old row. A typed private
  rejection preserves that selected prompt and recovers `edited`.
- The keyboard half of old
  `preparing_edit_ignores_duplicate_enter_and_only_the_matching_accepted_prompt_finishes_it`
  is `preparing_edit_ignores_three_more_enter_keys_and_keeps_the_newer_composer`
  in the same file. The actual Enter path submits one rewind. Three more Enter
  keys keep the same pending operation, selected prompt and newer composer;
  the actual local socket contains exactly the original rewind request.
  The backend worker keeps the matching-native-acceptance assertions in Core.
- Old `enter_during_initial_startup_does_not_reopen_the_helper_or_submit_an_edit`
  becomes `enter_during_initial_startup_keeps_the_rewind_composer_without_any_request`.
  The Device accepts numbered Starting facts, then receives Enter while an old
  prompt remains selected. It keeps `edit while starting` and creates neither
  a Client operation nor any socket command. Core keeps the native no-work guard.
- The label half of old
  `claude_process_parity_tests::activity_defaults_and_label_precedence_leave_context_revision_unchanged`
  extends the existing
  `remote/status_tests::native_transient_then_thinking_then_process_state_keep_the_existing_label_order`.
  Its table retains `Requesting`/43 and adds the original `Temporary`/42. Both
  check transient text, thinking text, Running, Requires action and Idle in order.
  Core keeps native parsing, invalid values and the unchanged context revision.
- The private half of old
  `clear_retains_process_work_but_failure_only_clears_activity_and_live_membership`
  is `remote/tasks_selection_tests::native_reset_and_closed_info_keep_the_exact_selected_process_and_private_composer`.
  It selects opaque task 1, keeps the exact composer `keep this draft`, accepts a
  numbered Reset retaining the task facts, then accepts Closed session facts.
  Both updates preserve the selected ID, retained task row and composer, with no
  mutation operation. Core keeps active membership, task-stop work and revisions.

The original edit/process files stay with the backend worker until its native
successors and these local counterparts have passed and joined the source tree.
All five local cases pass against the existing runtime; this slice adds no
runtime state, API or rendering policy.

## Three quota cases and the display clause

The old `src/tests/claude_quota_tests.rs` used `SessionState`, despite containing
no AppState. Its three native-window cases move to
`carlos-core/src/session/claude/quota_contract_tests.rs` and call the existing
session-private `ClaudeQuota` methods directly:

- `stream_fraction_is_percent_used_and_only_global_weekly_drives_the_weekly_badge`
  keeps the 25/40/90 percent buckets and the global weekly remainder of 60.
- `sparse_late_optional_read_cannot_replace_newer_native_windows_or_mix_model_buckets`
  keeps the newer stream window, optional five-hour/model buckets, exact
  revisions and unavailable-report removal rules.
- `missing_expired_nonfinite_or_wrong_unit_stream_values_remain_absent`
  keeps every original missing, expired and wrong-unit fixture and empty result.

The original string assertion `60%` now lives in
`weekly_usage_tests::the_shared_weekly_window_keeps_its_original_sixty_percent_label`.
That frontend test uses only `RateWindow`, `WeeklyUsage` and `WeeklyIndicator`.
The old quota file and its app-module hook can therefore be retired without
keeping a native session owner in frontend tests or exposing a new public API.

## Checks and source identity

The private checkout starts at `badb6e6`; its independent target is
`/tmp/carlos-ep019b/final-local-target`. Source hashes and final logs are recorded
in `/tmp/carlos-native-review-draft/final-local-proof.json`.

- The original phone case and its Device successor pass together.
- All three Device phone/keyboard cases, the status table and Reset/Closed case
  pass. The three Core quota cases and one frontend quota-label case pass.
- The full private no-default workspace, strict workspace/all-target Clippy,
  formatting and file/function size checks pass.

No failing-before runtime case was needed: every preserved local assertion
already passes on the existing Device. The source changes are test moves and
test additions. The final daemon cutover and joined release gates remain open.
