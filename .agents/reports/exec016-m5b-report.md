# Part 5b and preliminary fixes

Completed P1, P2, P3, P4, and part 5b in `/var/home/wegel/work/perso/carlos-daemon`, on `daemon-split`, starting at `d515ff9f6bde23868cecdb1338050a4c82e26036`. The final tree changes 38 Rust files. No commit, push, stash, reset, install, dependency change, toolchain change, `.agents/` edit, or `docs/` edit was made. The owner's main checkout was not touched.

## Separate patches and exact file ownership

The five patches below contain the exact hunks for each task. Apply them in the listed order to the starting commit. I applied them to a fresh archive under `/tmp/carlos-m5b/patch-proof/` and compared every Rust source with the working tree: they match exactly. Shared files therefore need not be separated by guessing at the final combined diff. See `/tmp/carlos-m5b/patch-files.json` and `/tmp/carlos-m5b/patch-proof.json`.

### P1: complete, `/tmp/carlos-m5b/p1.patch`

- `src/session/runtime_settings.rs`: add the private startup-preference field and borrowed accessor; initialize all current backend choices to `None` instead of treating preferences as confirmed choices.
- `src/app/backend_setup_launch.rs`: move preferences into the Codex and Claude constructors, then borrow and copy the small saved choices for the existing startup resolver; the native-history call change belongs to 5b.
- `src/app/backend_setup_grok.rs`: pass preferences into the constructor and obtain the same saved choices through the accessor for Grok startup.
- `src/tests/runtime_tests.rs`: include the new startup-preference tests.
- `src/tests/runtime_tests/runtime_settings_parity.rs`: keep the constructor test and change its expected facts to unconfirmed current choices plus the exact separately retained preferences.
- `src/tests/runtime_tests/startup_preferences.rs`: enumerate missing, malformed, wrong-type, empty, and valid preference files across all three backends and new/resume/continue; also check Codex with absent native choices.

P1 keeps the resolver, model-catalog rules, native settings parser, and file reader unchanged. It stores raw constructor preferences separately; the existing resolver and setters still choose and trim effective values as before. The startup tests check concrete model/effort/summary results, including Codex native choices on resume/continue, saved choices for a new Codex thread, Claude inheritance, and Grok fallback choices. Existing catalog and capability tests still cover incompatible saved models and effort clamping.

### P2: complete, `/tmp/carlos-m5b/p2.patch`

- `src/session/model_info.rs`: hold the plain `ModelInfo` type, with no backend or terminal dependency.
- `src/session/mod.rs`: register `model_info`; the input-history registrations belong to 5b.
- `src/session/runtime_settings.rs`: import `ModelInfo` from the plain session module.
- `src/protocol_params.rs`: re-export `ModelInfo` so existing callers need no unrelated import churn.

No other type from `protocol_params` is needed by `src/session/`; an exhaustive search finds no remaining reference to that module in the session tree.

### P3: complete, `/tmp/carlos-m5b/p3.patch`

- `src/session/runtime_settings.rs`: expose the existing trimming helper for the settings editor; keep its body unchanged.
- `src/app/runtime_settings_model_settings.rs`: use that helper, remove the duplicate, and rename the read-only choice builder to `saved_choices(&self, ...)`.
- `src/app/state_settings.rs`: call `saved_choices`, remove the stale `allow(dead_code)`, and route the apply path through the already used app queue method.

The starting commit already had one close on the apply path: `AppState::apply_model_settings` closes the overlay after queuing choices. The view choice builder did not close it. I kept that single close; removing it would change behavior.

### P4: complete, `/tmp/carlos-m5b/p4.patch`

- `src/tests/protocol_history_transport_tests.rs`: add peer receipts and a test-only helper that ends each unanswered history call's reply wait by dropping its registered sender after receipt; retain the cap, ordinary-control, late-drain, and recovery assertions.

The peer sends the exact received request id on the event channel. The helper removes a `Target::History` sender only after that receipt. Dropping it makes `recv_timeout` return its disconnected error, which enters the same `Err(_)` branch as a deadline in `AppServerClient::call`. That branch retires the caller without releasing its undrained large-reply allowance. The helper waits for the actual call to return and checks its timeout text and open connection. The 65th actual call must then fail with `64 undrained history reads`. An ordinary `model/list` still succeeds while the peer drains all 64 late responses, and a further large read succeeds. If the call stopped registering history targets, released grants on retirement, closed the connection, failed to cap grants, or failed to reclaim them on late replies, this test would fail.

The 30-second receipt/call limits are deadlock guards; elapsed time no longer chooses the tested path. No production protocol code changed.

- 300 normal repeats: **0 failures**, 7.751 seconds total.
- 100 repeats while another Rust 1.100.0 release build ran in `/tmp/carlos-m5b/load-target`: **0 failures**, 6.747 seconds total. The script checked that the build remained alive before every repeat and after the last repeat; that build also finished successfully.
- The isolated rewritten test took about 20 ms, compared with the former roughly 1.3 seconds.
- Evidence: `/tmp/carlos-m5b/p4-normal.json`, `p4-normal-failures.json`, `p4-load.json`, `p4-load-failures.json`, and `p4-load-build.log`.

### Part 5b: complete, `/tmp/carlos-m5b/m5b.patch`

- `src/session/input_history.rs`: own prompt text, optional `EntryId` links, duplicate-entry keys, entry-to-turn links, and optional native history; return whether recording/discarding requires a cursor reset.
- `src/session/input_history_commands.rs`: expose borrowed prompt facts and explicit session writes, including reset and test-only retention.
- `src/session/input_history_native.rs`: move the existing exact native-position ledger; keep its maps private and native-position fields visible only inside the session tree.
- `src/session/state.rs`: construct and own `PromptHistory` with session-only field access.
- `src/session/mod.rs`: register the three history modules; the `model_info` registration belongs to P2.
- `src/app/input_history_state.rs`: retain only navigation index, selected entry id, saved drafts, rewind mode, and rewind restore draft in `InputHistoryView`; borrow prompt records when navigating.
- `src/app/input_history_native.rs`: remove the old app-owned copy after moving it into the session.
- `src/app/view_state.rs`: construct and own the history view.
- `src/app/state.rs`: remove the mixed history field, read rewind mode from the view, and adapt test accessors to the two owners.
- `src/app/state_input.rs`: route prompt writes through session methods, apply their cursor-reset result, navigate with borrowed records, and build the native rewind request from view selection plus session facts.
- `src/app/state_thread.rs`: reset both owners at the same history/fork points and restore turn links and native positions through session methods.
- `src/app/backend_setup_launch.rs`: enable native Codex history through the session; the saved-preference changes belong to P1.
- `src/app/item_history_draft.rs`: import the moved native ledger and read the native-history flag through the session accessor.
- `src/app/claude_notifications.rs`: record acknowledged prompt-to-turn links through the session.
- `src/app/claude_prompts.rs`: record local-echo and accepted human prompt links through the session.
- `src/app/claude_session_history.rs`: record loaded human prompt links through the session.
- `src/app/turn_submit.rs`: obtain the same original-text native anchor through the app's split-owner helper.
- `src/app/state_summaries.rs`: update only the test reference-invariant check to read session history; do not change tool-label behavior.
- `src/app/mod.rs`: remove the old native-ledger module and register the new history tests.
- `src/tests/codex_question_support.rs`: enable native history through its new owner in shared test setup.
- `src/tests/codex_reconnect_identity_tests.rs`: adapt history setup and native-anchor reads; retain every assertion.
- `src/tests/codex_steer_tests.rs`: adapt history setup, enabled checks, and native-anchor reads; retain every assertion.
- `src/tests/entry_history_identity_tests.rs`: adapt native-history setup and selected-anchor reads; retain every assertion.
- `src/tests/entry_lookup_extra_tests.rs`: adapt native-history setup and selected-anchor reads; retain every assertion.
- `src/tests/entry_review_history_tests.rs`: adapt native-history setup, prompt count, and selected-anchor reads; retain every assertion.
- `src/tests/input_status_tests.rs`: read the selected native request through the split-owner helper; retain the Claude assertion.
- `src/tests/input_history_split_tests.rs`: add four enumerated tests for navigation, rewind restore/clear, recording/discarding, and replacement during rewind.
- `src/tests/input_history_split_fork_tests.rs`: add two enumerated tests for exact backend fork targets and hidden native user positions in live/replaced history.

## Parity ledger for 5b

Listed as method-to-owner mappings to follow the repository's no-table rule. All rules below were checked against the starting commit.

- `InputHistoryState::new`: split into `PromptHistory::default` in the session and `InputHistoryView::default` in the view; all vectors/maps/sets start empty, native tracking starts disabled, and every cursor/draft starts absent.
- `enable_native` / `native_enabled`: session command plus borrowed accessor; enabling remains idempotent and does not discard existing positions.
- `record_native`: session command plus ledger method; disabled tracking returns `None`; exact `(turn, item)` keys deduplicate started/completed notifications; the first unique item gets zero and increments that turn's count once; `None` visible entry still consumes its native position.
- `selected_native`: session `native_user(entry_id)` after resolving the view's selected id; absent selection/native ledger/link still returns `None`.
- `selected_native_request`: app `rewind_native_request` plus session `native_request(index, entry)`; require both cursor and selected id, then the native link and saved text; preserve original text, exact item id, and zero-based position independently of the edited composer.
- `reset_navigation`: view only; clear cursor, selected id, and saved navigation draft, without changing rewind mode or its restore draft.
- `enter_rewind_mode`: view only; already active returns false before changing anything; otherwise save the composer, mark rewind active, and reset navigation; the app still disables auto-follow before navigating up and keeps the Claude hint order.
- `exit_rewind_mode_restore`: view only; inactive returns `None`; otherwise take the restore draft with empty-string fallback, leave rewind, reset navigation, and return the draft; app viewport, Claude focus, and in-flight-edit guards remain in their original order.
- `clear_rewind_mode_state`: view only; clear the mode and rewind restore draft while retaining navigation and its draft; the app keeps its existing failed-edit cleanup and viewport behavior.
- `push_history`: app supplies an absent entry id to the session record method, then resets navigation only when that method reports the original reset case.
- `discard_pending_history`: session checks only the final prompt's exact text and absent entry id; only a match pops both vectors and requests a view reset; every other case remains a no-op.
- `record_history`: session; empty text records nothing but requests a cursor reset; a repeated entry id returns without reset; a new id enters the duplicate set before the matching unbound tail can be backfilled; otherwise append text and optional id together; successful backfill/append requests reset.
- `navigate_up`: view borrows session history; empty history returns before saving a draft; zero stays zero, another cursor decrements with the same saturation, and no cursor saves the composer and selects the last prompt; update selected id alongside cursor and return only that prompt's text.
- `navigate_down`: view borrows session history; absent cursor returns `None`; another record advances cursor and selected id; the end takes the saved draft with empty-string fallback and clears cursor/selected id; rewind mode remains unchanged.
- `rewind_selected_entry_id`: view stores the selected `EntryId` when its cursor changes; recording/reset clears both together, duplicate records preserve both, and test-only retention refreshes the id after dead links are cleared.
- `rewind_selected_history_index`: view returns the same optional prompt cursor; the app retains Claude's unknown-source `usize::MAX` case.
- `history_len`: session returns the prompt-record count, including an unbound submitted/queued tail exactly as before.
- `record_turn_id`: session command overwrites the same entry's fallback turn link with the supplied turn id.
- `selected_turn_id`: app still prefers a selected Claude unknown-source UUID; otherwise the session prefers the selected entry's native turn over its fallback turn link; absent selection/link returns `None`.
- `retain_entries` (test-only): session clears dead optional ids and removes dead recorded/turn/message links; native counts and native item positions remain intact; the app refreshes the view's selected id after this step.
- `message_ids` / `assert_invariants` (test-only): session retains the same borrowed ids and the same prompt/native-ledger assertions; no test assertion was removed.
- `set_rewind_selection` (test-only): view sets rewind mode and the supplied cursor without changing drafts, and resolves its optional entry id with the same bounds behavior.
- `record_input_history` / `record_input_turn_id`: app still resolves transient transcript offsets to `EntryId` once; the temporary native turn scope is restored before fact writes; record fallback turn first, native position second, then change visible steer kind and dirty the same row only if its kind changes.
- `replace_history_draft` / `reset_thread`: app invokes the session history reset at the old reset point and resets the history view there too; native tracking stays enabled when it was enabled; transcript adoption, historical records, row-kind changes, cache resets, viewport resets, approval handling, and dirty reports keep their order.

## Tests and previous-commit comparison

The six new 5b tests ran unchanged against the previous commit after adapting only the new accessor names back to the old names. All six passed on both trees, in under 0.01 seconds per focused run. The prior-tree log is `/tmp/carlos-m5b/previous-history-tests.log`; the current focused log is `history-tests.log`.

- Navigate through seven explicit up/down steps, including absent cursor, oldest clamp, newest restore, and empty history.
- Enter rewind twice, edit the composer, and exercise both cancel/restore and clear-with-cursor-retained, followed by navigation.
- Enumerate duplicate id, empty text, append, bound-tail discard, mismatched discard, pending-tail backfill, and pending-tail discard, with fixed cursor and prompt-count expectations.
- Replace history while rewinding with either empty or visible prompt text, then verify focus reset, composer retention, and up/down draft restoration.
- Enumerate Codex, Claude, and Grok fork calls with exact thread, keep/drop counts, selected turn, optional exact native anchor, original text, and subsequent turn/start parameters. Verify Claude remains in its asynchronous edit state and restores the draft; verify Codex/Grok navigate the accepted fork history afterward.
- Enumerate zero, one, and three invisible native users across live and replacement history. Live inputs repeat started/completed/completed. Assert the one visible prompt's exact native position and plain/steer kind.

Existing tests also continue to check Claude steering UUIDs, Codex steering refusals and authoritative paged rewind checks, reconnect identities, Grok prefix preservation, failed edits, model overlays, settings persistence, and transcript reference invariants. I did not change their expected values. The constructor test's expected current choices changed only to assert P1's explicitly requested distinction.

The reused live-event harness ran a fixed random sample of **3,000** of its 24,300 streams, with Python RNG seed **516052**, against the previous commit. The sample contains 980 Codex, 994 Claude, and 1,026 Grok streams. I added plain test-only snapshots in copied source trees so the oracle also compares prompt text/id links, duplicate keys, fallback turns, and all native position/count/message maps, not just transcript rows.

- **54,587 event checkpoints matched exactly.**
- **0 differences** in facts, backend calls, dirty/redraw log, rendered lines/styles, or checkpoints.
- **0 panics** on either tree.
- The harness ran only in `/tmp/carlos-m5b/{previous,current}` and its added methods/hooks did not enter the working tree.
- Evidence: `/tmp/carlos-m5b/comparison.json`, `sample-manifest.json`, `sample.jsonl`, `hashes.json`, and `previous-*.jsonl` / `current-*.jsonl`.

The random harness does not exercise composer up/down navigation or real startup preference-file reads. The enumerated tests above cover those paths. This task's reference is the requested previous commit; I did not rerun the older pre-plan-to-current comparison, whose already documented identity changes and idless-item panic belong to earlier milestones.

## Required checks

- `cargo build`: passed.
- `cargo build --release`: passed with default features and Rust 1.100.0.
- `cargo fmt --check`: passed, also rerun by the terminal-check script after the final comment changes.
- `bash scripts/check-rust-style.sh --all`: passed; the changed files are at most 355 lines and changed/new functions are at most 59 lines.
- `cargo test`, first run: **882 passed, 3 ignored, 0 failures**, test body 3.77 s, command 3.945 s.
- `cargo test`, second run: **882 passed, 3 ignored, 0 failures**, test body 3.78 s, command 3.931 s.
- `bash scripts/test.sh`, one run: passed, including **174 terminal workflows**, socket-recorder checks, package smoke checks, and its Rust suite; terminal workflows took 79.474 s and the complete script took 114.805 s.
- `git diff --check`: passed.
- An exhaustive search found no `crate::app`, ratatui, crossterm, or `protocol_params` reference under `src/session/`.
- The only working-tree compiler warning was the permitted existing atomic deprecation in `protocol_writer.rs`.

Check logs and machine-readable results live under `/tmp/carlos-m5b/`. Terminal artifacts live under `target/ui-artifacts/run-N6jT15/`. No test failed in either final Rust run or the terminal script, and the rewritten P4 test passed all 400 requested repeats.

I also ran seven alternated previous/current release-binary pairs for both recorded perf cases after the checks finished, using the same compiler and default release profile. Medians in milliseconds, previous/current:

- Synthetic, 6,001 messages and 39,347 rendered lines: replay elapsed 48.98/48.53; replay apply 15.08/15.26; full layout 15.21/15.23; full draw 3.86/3.50; scroll p50 0.71/0.71; typing p50 0.60/0.60; working p50 0.60/0.60; append p50 0.60/0.61.
- Fixed history fixture, 5 messages and 9 rendered lines: replay elapsed 1.14/1.10; replay apply 0.01/0.01; full layout 0.01/0.01; full draw 0.50/0.50; scroll p50 0.44/0.45; typing p50 0.37/0.37; working p50 0.37/0.37; append p50 0.40/0.39.
- No median regressed by 10%. Evidence: `/tmp/carlos-m5b/quiet-perf-medians.json` and `quiet-perf-runs.json`. An earlier run overlapped tests/build work and was discarded for the final speed comparison.

## Small edits outside the Milestone 5 allowed list

- `src/protocol_params.rs`: P2 requires moving its type definition; retain a re-export and touch no builders or parsers.
- `src/app/claude_notifications.rs`: redirect one prompt-turn write through the new owner.
- `src/app/claude_prompts.rs`: redirect two prompt-turn writes through the new owner.
- `src/app/claude_session_history.rs`: redirect one loaded prompt-turn write through the new owner.
- `src/app/item_history_draft.rs`: update the moved type's import and native-enabled read.
- `src/app/state_thread.rs`: split the old mixed reset and redirect history-restoration writes; this is the existing transcript replacement/fork entry point.
- `src/app/turn_submit.rs`: replace one direct selected-native-request read with the split-owner helper; leave every backend call and guard unchanged.

Every other changed path is in the allowed app/session families or is an affected settings/history test under `src/tests/`.

## Discoveries and decisions for the integrator's plan notes

- A disconnected reply channel and a deadline use the same `Err(_)` retirement branch in the existing blocking Codex call. A receipt followed by sender drop can therefore test undrained history bounds without changing production clocks or sleeping.
- A duplicate prompt entry must return without resetting view navigation. A simple unconditional reset in the app would have changed behavior; the session now returns the exact original reset decision.
- Clearing rewind mode deliberately leaves the existing navigation cursor and its saved draft intact. Exiting with restore clears them. Keep that distinction for later clients.
- The native ledger retains position/count records when test-only transcript retention removes visible links. Hidden and duplicate native items therefore preserve the same rewind positions after this move.
- The app still coordinates transcript replacement and prompt-history reset synchronously in their original order. A later daemon entry point should account for both owners; this part adds no stronger atomicity, stale-id validation, or fork guarantee.
- The session may hold an unbound prompt record after the app accepts a submitted/queued command; rejected pending tails are removed by the existing failure path. Unsent composer edits and navigation/rewind recovery drafts remain in the view.
- No transcript-sized clone, render loop, row loop, delta loop, or new backend request was introduced. Prompt navigation copies only the selected text as before; native lookup and cursor/id changes stay constant-time.
- Parts 5c and 5d remain untouched. The settings edits are limited to P1 through P3. No new UI workflow broke, so the existing terminal workflows provide the terminal regression checks.

No requested work remains unfinished. The scope limits of the random oracle are stated above; the focused enumerated tests cover those paths.
