# Queue tests at the Core and Device owners

The eight old `src/tests/codex_queue_tests.rs` tests mixed native queue state with
one terminal's composer, selection and recovery text. Their successors use the
existing Core queue functions, `TestBackend`, and the real socket-terminal Device.
No test-only terminal runtime or production test constructor was added.
The Core fixture enables native prompt-history tracking, as the old helper did.

## Old assertions and their successors

- `queued_human_enters_history_only_after_native_consumption_and_duplicate_sends_are_blocked`
  keeps its native assertions in `codex/queue_contract_tests.rs`: one exact
  `thread/queue/add` request, no message or prompt record at acknowledgement, one
  record after the consumed native item, and no duplicate write. The unchanged
  daemon tests `test_daemon_queue_f3_adds_to_native_queue_during_a_held_turn_without_chat_echo`
  and `test_daemon_queue_consumed_native_humans_precede_ralph_and_preserve_the_composer`
  retain the empty-composer, no local echo and human-before-Ralph assertions.
- `stale_and_old_epoch_pages_cannot_authorize_automatic_work_or_replace_a_complete_snapshot`
  moves to `codex/queue_read_contract_tests.rs`. It checks the read revision, native
  epoch, root, exact cursor/limit and authoritative empty-queue gate. The additional
  read-guard case checks disabled, closed, blocked, pending, known and failed reads.
- `queue_failure_and_eof_preserve_main_and_submitted_drafts_without_a_retry`
  splits between Core's one-time recovery/EOF test, the actual Device's
  `unrecovered_queue_text_blocks_another_add_without_replacing_either_draft`, and
  the unchanged daemon tests
  `test_daemon_queue_rejection_keeps_a_newer_composer_and_another_terminals_draft`
  and `test_daemon_queue_restart_keeps_uncertain_input_private_and_never_replays_it`.
  These retain the submitted text, newer composer, first recovery, one native
  write, explicit recovery and no replay assertions.
- `attachments_and_text_spans_are_never_lost_by_the_plain_text_editor` keeps both
  exact native input values in Core. Neither image attachments nor text spans
  become editable plain text; starting them preserves every native field. The
  unchanged `test_daemon_queue_reads_complete_large_text_and_preserves_rich_native_inputs`
  checks the real editor refusal and paged full-text path.
- `an_external_edit_or_consumption_preserves_the_draft_and_does_not_retarget_selection`
  moves its revision, original input, stale edit and exact selected native delete
  assertions to Core. The unchanged
  `test_daemon_queue_two_terminals_keep_paged_selection_and_stale_editor_drafts_private`
  and `test_daemon_queue_edits_moves_drops_and_starts_only_the_selected_native_input`
  keep the two private drafts, retired selection and explicit reselection clauses.
  `test_daemon_queue_phone_resize_keeps_both_editors_and_ignores_tiny_save_and_paste`
  keeps the phone editor, tiny paste/save and main-composer clauses.
- `invalid_queue_pages_keep_automatic_work_blocked_instead_of_installing_partial_history`
  moves to Core with its repeated entry/cursor and exact continuation token.
  A second case checks repeated cursors, the 4096-entry bound and 16 MiB bound.
  None publishes partial entries or authorizes automatic work.
- `a_fork_cannot_discard_an_unsettled_queue_submission_or_an_unrecovered_draft`
  keeps both Core guards through `prepare_rewind_text`. A new actual Device test
  checks its private recovery guard before a socket command is created. Another
  Device test explicitly recovers the text, then submits exactly one rewind with
  the same captured prompt ID and edited text.
- `later_queue_pages_use_the_remaining_whole_snapshot_deadline` keeps the original
  19-second/21-second schedule in Core. The continuation uses at most 1000 ms;
  the expired snapshot sends no third native request.

The removed `codex_queue::{reject_switch,page,allows_automatic}` functions were
test-only forwarding functions with no callers after this move. The real Core
functions and terminal queue code remain.
The combined native/queue move also removes the final
`AppState::submitted_turn_count` test forward: both old callers now assert the
real Core prompt-history count. That last-caller deletion follows the backend
native test prerequisite and stays out of its earlier standalone prefixes.

## Private recovery guard

The new Device test exposed a missing socket-terminal check. The old terminal
refused to fork while its queue recovery text remained unrecovered. The socket
terminal instead submitted the rewind and cleared the edited composer. The
daemon cannot enforce this local rule because another terminal's recovery text
is private.

`Device::submit_rewind` now checks this device's existing queue recovery field
before creating an operation. The test proves that a blocked fork keeps its
composer, captured selection and recovery text, creates no Client operation and
writes no command. Explicit recovery releases the guard. No protocol or Core
state was added.

## Evidence and limits

The private checkout starts at `8e07c18`. It includes only this queue slice and
the already committed `64430e6` removal of the unused `input_history_len` forward
from `src/app/state.rs`. That predecessor hunk is excluded from the queue commit.
The private Cargo target is `/tmp/carlos-ep019b/queue-contract-target`.
The primary combined check also covers the final shared forwarding-method
deletion, which is still needed by the old backend tests in the private base.

- Ten new Core cases pass; the queue-filter run passes 27 cases.
- The causal Device run before the guard passes one case and fails the fork
  case because the real Device returns success. The three Device cases pass
  after the guard, including zero-command and explicit-recovery checks.
- Eight unchanged actual daemon queue workflows pass in 6.122 seconds.
  Their frozen matching pair is `/tmp/carlos-ep019b/process-identity-pair`, source
  marker `9a66816a2ffa2e849eabe2479c1c73be`. This is an earlier declared combined
  source pair, not a claim that those executables came from the final queue commit.
- The private full workspace, strict no-default workspace/all-target Clippy,
  formatting and source-size checks pass. The largest touched file has 235 lines;
  the longest touched function has 49 lines.

`/tmp/carlos-native-review-draft/queue-proof.json` records source hashes, binary
hashes and log paths. The Device negative is `queue-private-device-before02.log`;
the earlier `before01` is only a corrected fixture type-annotation error.
The final `queue-device-exact-source.json` capsule also freezes both test
executables and hashes the same test source with and without the three-line
guard. Its exact fork case fails before the guard; all three cases pass after
restoring the checked source. The empty-filter setup log is discarded evidence.
The initial private Clippy run found an unused forwarding method in the older
base; the final check includes the exact committed predecessor hunk above.
The final `queue-native-history-{tests,workspace,clippy}.log` checks also include
the corrected native-history fixture flag: 27 queue-filter cases and 1584
workspace tests pass, with nine existing ignored tests.
The final default cutover, joined UI gate and EP review remain separate work.
