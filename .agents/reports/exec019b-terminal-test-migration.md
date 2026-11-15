# EP019b terminal test moves

These batches preserve useful assertions while removing the frontend tests' need to construct a native session owner. None closes the whole EP or authorizes the default launcher cutover.

## Core runtime and borrowed editor batch

- Move thirteen Ralph marker, toggle, continuation-deadline and human-queue-order tests from runtime_tests/ralph_runtime.rs and picker_and_local_history.rs into carlos-core session/runtime_behavior_tests/ralph.rs. The cases call the same SessionState methods directly and keep their original text, clock boundaries, native state and queue assertions.
- Move five settings rule matrices into core runtime_behavior_tests/settings.rs. Preserve omitted versus null versus empty native values, stale rejected choices, capability changes, complete pending-choice replacement and default-summary rules. The native reply cases call the actual core observed/reject functions. Add the exact default-first stable catalog-order assertion at its actual core owner.
- Move the three settings-editor matrices to runtime_settings_editor_tests.rs using RuntimeSettingsView and borrowed SettingsFacts/model rows. Keep model cycling, pending-choice preference, effort fallback, Unicode edits, cancel and supported-summary assertions. The former apply-model wrapper checks now inspect the actual borrowed editor's saved choices; actual daemon Save behavior remains covered by the settings popup workflows.
- Remove only the now-unused test-only AppState delegates and imports. Keep production wrappers until the separate runtime deletion commit.

## Device keys and dictation batch

- Escape: preserve the old quiet-after-interrupt timing sequence on ViewState; route held Escape and pasted composer text through Device.event. The failing-before run has two failures, and all three cases pass after the two event-hook fixes. No interval changed.
- Dictation: Device.event covers Ctrl+D start/stop/restart, Enter stop, Escape cancel, F9 captions and the settings endpoint field. Assert that these gestures create no client operations and keep the private composer.
- The shared local-view tests retain partial/final insertion, matching capture auto-stop/audio, matching capture error, missing model path and profile-picker assertions. Existing view tests retain request/capture identity, late-final rejection, bounded event draining, cancellation on drop and F7 profile changes.
- Retire the old AppState dictation delegates/tests after those successors compile. The old merged-loop prioritizer test remains until the loop itself is removed. Actual default/no-default microphone-free terminal cases remain tests/ui/test_daemon_dictation.py; physical capture and inference are still untested.

## Evidence

- Focused core runtime: eighteen original cases pass; the catalog-order successor is added afterward.
- Focused borrowed editor: four tests pass.
- Default and no-default strict all-target lint pass. The full workspace run reaches888 passing frontend cases before the backend worker's in-progress fatal-write fixture; that unrelated fixture now passes on the worker's fresh build. A final workspace run remains required before committing.
- The exact4cf7912 prefix plus the corrected readonly queue-caption fixture passes the full script: 1539Rust cases,143daemon,296terminal,2selection,5source-failure,2dictation and23native-monitor cases, plus installed Codex contracts, helper, architecture, dependency, package and recorder checks. Log /tmp/carlos-ep019b/terminal-parity-index-gate-retry.log; artifacts run-5sMH0x under the recorded private checkout. That runner still mixes embedded and socket workflows; it does not replace the complete all-daemon gate.
