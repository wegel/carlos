# Keep failed edit text once, and keep queued human text separately

The socket terminal used one recovery helper for edits and queued human sends.
That helper saved another draft whenever the composer was nonempty, even when
a failed edit's exact text was already there. The old terminal kept that edit
once, while deliberately retaining an identical queued human send separately.

`Device::retain_failed_edit_text` now handles only an operation already known to
be an edit. It compares the submitted text with the composer's borrowed lines,
including Unicode and a trailing newline. If the text is already present, it
releases only that operation. Otherwise it calls the unchanged generic helper.

Only two existing edit-specific callers use it: a matching
`apply_history_operation` result and the pending edit retired by a replaced
daemon greeting. The generic send path still retains an identical queued human
input for explicit recovery. No operation-kind getter, metadata or public
runtime API was added.

## Exact cases

The real Device recovery tests cover:

- A failed Rewind whose exact text is already in the composer creates no saved
  duplicate. Its failure stays visible after an unrelated acknowledged command;
  clearing the composer and pressing F5 cannot recover another copy.
- An identical queued Send remains separately recoverable after the composer
  is cleared. A different newer composer also keeps the original failed edit
  available exactly once.
- A real replacement greeting applies the same edit rule: identical text stays
  once; different text remains saved separately.
- An explicit later ordinary Send clears the old edit-failed badge, accepts its
  matching result, and leaves the distinct old edit recoverable through F5.

These preserve the local clauses of
`recovery_keeps_identical_composer_and_unrelated_ack_cannot_clear_its_failure`
and `a_successful_later_send_dismisses_the_failure_badge_but_keeps_saved_drafts`.
The backend worker retains the native submission/recovery-origin assertions.

The old `src/tests/client_recovery_tests.rs` cases keep their exact names under
`history_client_recovery_tests.rs`:

- `two_clients_apply_the_same_correlated_recovery_without_overwriting_newer_text`
  keeps the origin's `typed ahead` and saves `submitted edit`. The other device
  keeps `second device draft`, saves nothing, and retains its own pending text
  and Pending operation state unchanged.
- `empty_originating_editor_recovers_but_empty_other_editor_stays_empty` puts
  `submitted` only in the empty originating editor. The empty foreign editor
  and its unrelated pending record remain unchanged.

Both devices have distinct real greeted Client IDs. Each receives the same
encoded `operation/updated` bytes through the production codec; the foreign
client rejects the result at its identity guard. The test-only widget helper
accepts an explicit Client ID, while its existing default remains unchanged.
The backend worker can retire the old two cases and their shared legacy support
after the new tests join, together with the remaining native edit/process moves.

## Evidence

The private checkout starts at `df300aa`, with its own Cargo target at
`/tmp/carlos-ep019b/edit-recovery-target`.

- The causal five-case run before the fix passes three and fails two, on the
  duplicate edit after a result and after a replacement greeting. The old send
  and different-composer recovery cases pass.
- All eight final recovery cases pass, including both Client-ID checks and the
  actual later Enter/send/request/result path.
- Strict no-default workspace/all-target Clippy, formatting and size checks
  pass. The largest touched source has 355 lines; the longest function has 60.

`/tmp/carlos-native-review-draft/edit-retention-proof.json` records the before
source copies, frozen test binary, hashes and logs. The initial import-path
compile error is setup evidence, not a runtime negative. The final source and
normal commit hook are checked separately before handoff.

This fixes a private draft rule. The final daemon cutover and whole-EP checks
remain separate work.
