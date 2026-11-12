I completed P1, P2, and part 5c in `/var/home/wegel/work/perso/carlos-daemon`, starting at `94ba37f1c6a6ae0e9bfb176c90722a480380b99d` on `daemon-split`. I did not access the owner's checkout, commit, push, stash, reset, install a binary, change the Rust override, or edit `.agents/`, `docs/`, dependencies, preferences, or cache code. The checked builds use Rust 1.100.0 and default features.

**Patches and exact replay**

Apply `/tmp/carlos-m5c/p1.patch`, then `p2.patch`, then `m5c.patch`. I applied those patches to a fresh archive of the starting commit and compared 1,215 tracked/new paths with the working tree: no missing, additional, or different files. The ordinary Git index remains unchanged. `patch-proof.json` records the base, order, and patch hashes; `final-source-hashes.json` records the checked source bytes. Part 5c has 58 diff entries, including 12 moves.

**P1: test the real new-thread override**

- `src/tests/runtime_tests/startup_preferences.rs`: drive the saved-preferences/new-Codex case through the real `configure_codex_app`; give the peer native/high values and advertise both native/saved models with both high/low efforts, then require saved/low plus the native summary and the exact deferred change.
- `src/app/backend_setup_launch.rs`: add only a `#[cfg(test)]` bridge that constructs the native launch response and calls `configure_codex_app`.
- `src/app/backend_setup.rs`: re-export that bridge only for tests.

The test passes. Temporarily deleting the saved-model override makes it fail; independently deleting the saved-effort override also makes it fail. I restored both lines. See `p1-test.log`, `p1-missing-model.log`, and `p1-missing-effort.log`. P1 changes no production code.

**P2: count the failure, then remove the test's scheduling assumptions**

The measurements below use the unchanged commits and Rust 1.100.0. I used detached worktrees `/tmp/carlos-m5c-preplan` and `/tmp/carlos-m5c-head`. The full-suite batches ran concurrently to expose scheduling faults; their failure rates should not be treated as estimates for an idle machine. `baseline-results.json` gives the batch counts and every captured failure message.

- Pre-plan `a59221c`: isolated flood test 2/200 failures; flood test inside full `cargo test` 24/235 failures.
- Starting HEAD `94ba37f`: isolated flood test 0/200 failures; flood test inside full `cargo test` 27/218 failures.

Both commits report `assertion left == right failed: reserved terminal diagnostic`, `left: 0`, `right: 1`.

The old test assumes an order the producer does not promise:

1. The reader fills the 4,096-frame budget and calls `ClientState::close`.
2. `close` marks the connection closed, kills its process, and sends the pending caller its error.
3. The awakened test drains the event queue with `try_recv` and sees no closure notice yet.
4. The reader then enqueues the reserved closure notice.

Carlos does not lose the notice. The test incorrectly treats a caller's reply as proof that the event producer has finished. This is a test observation race, not a production race requiring a changed delivery guarantee. Milestone 3 did not introduce it: `protocol_state.rs`, `protocol_events.rs`, and this test have no diff between `a59221c` and `ac02f0a` (`p2-preplan-vs-m3.diff`). I preserved production ordering.

- `src/tests/protocol_transport_tests.rs`: receive and assert every progress frame from 0 through 4,095 after the call fails, receive the reserved closure notice, check its backlog reason, then require no additional event and verify process exit. Replace the peer's sleep with a blocked input read. Thirty-second waits are watchdogs; elapsed time does not select the expected path.
- `src/tests/protocol_stop_tests.rs`: repair the additional timing fault described below with a peer receipt, an explicitly expired pending request, and a peer that sends its late stop reply before answering the next read. The test still proves the exact stop target, continued native progress, healthy reads, no retry, and suppression of the late reply.
- `src/protocol.rs`: register that stop test under the protocol module, only with `#[cfg(test)]`, so it can expire the existing private pending request without a production accessor.
- `src/tests.rs`: remove the old registration of the same test; no test was deleted.

The first 60-suite check of the flood-only repair failed once, on run 59, in `app::tests::protocol_stop_tests::targeted_stop_timeout_keeps_native_progress_and_reads_alive_without_retry_or_late_reply`: `assertion failed: progress && error` at line 45. That run took 12.60 seconds while builds ran. Its peer slept 200 ms and its request used a 40 ms deadline; the assertion did not record which two events won that race. I removed those scheduling assumptions without changing the production timeout path. The test is now registered as `protocol::stop_tests::targeted_stop_timeout_keeps_native_progress_and_reads_alive_without_retry_or_late_reply`.

After both repairs, I ran the full `cargo test` **60 times, with zero failures of any test**. Each run had 882 passed and 3 ignored. The frozen P1/P2 tree is `/tmp/carlos-m5c/p2-checkout`; `p2-final-60/suite-results.json` and all 60 logs preserve the result. P2 changes no production behavior.

Other failures observed in the unchanged baseline batches follow. These are counts of failed test invocations, so several may occur in one suite. They did not recur in the final 60-suite check.

- `claude_backend::transport_tests::slow_ui_stops_the_helper_and_settles_unanswered_requests`: HEAD 0; pre-plan 24. Message: `assertion failed: queued.iter().any(|line| line.contains("backlog overflowed"))`.
- `protocol::history_transport_tests::late_large_history_replies_drain_without_closing_healthy_native_work`: HEAD 3; pre-plan 8. Message: `healthy next read: Codex stdin deadline expired; request may already have been accepted`.
- `protocol::history_transport_tests::unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls`: HEAD 0; pre-plan 87. Message: `Codex stdin deadline expired; request may already have been accepted`.
- `protocol::tests::a_broken_stdin_pipe_settles_the_caller_and_reports_the_actual_write_error`: HEAD 0; pre-plan 3. Message: `Timeout waiting for Codex turn/start`.
- `protocol::tests::a_short_queue_read_deadline_discards_late_reply_without_stopping_native_work`: HEAD 0; pre-plan 10. Message: `healthy after read timeout: Codex stdin deadline expired; request may already have been accepted`.
- `protocol::tests::a_slow_read_does_not_kill_native_work_or_leak_its_late_reply`: HEAD 2; pre-plan 14. Message: `still healthy: Codex stdin deadline expired; request may already have been accepted`.
- `summaries::tests::worker_tests::cached_and_uncached_history_do_not_wait_for_model_or_trigger_calls`: HEAD 1; pre-plan 1. Message: `assertion 'left == right' failed   left: ""  right: "x"`.

**Part 5c: per-file changes**

- `src/app/backend_setup.rs`: pass the launch cwd through the session method; P1 also adds its test-only re-export.
- `src/app/input_events_model_settings.rs`: edit the view checkbox and persist the accepted session preference after apply.
- `src/app/item_history_draft.rs`: import the existing plain read parser directly from the session.
- `src/app/mod.rs`: route plain helper modules to their session owners; retain test-only aliases where existing tests use them.
- `src/app/models.rs`: keep the now-test-only ToolOutputDetails re-export behind cfg(test), avoiding a new warning.
- `src/app/overlay_render_model_settings.rs`: draw the unsent checkbox from the view.
- `src/app/perf_synthetic.rs`: import the plain read formatter directly from the session.
- `src/app/state.rs`: load the saved label preference once in the app, pass it into both owners, and remove the mixed summaries field.
- `src/app/state_input.rs`: retain live summary bindings through a session command in the existing test rewind helper.
- `src/app/state_settings.rs`: initialize the unsent view checkbox from the accepted session preference.
- `src/app/state_summaries.rs`: keep the history closure, native scope adapter, redraw calls, and test adapters; delegate label facts and worker ownership to the session.
- `src/app/state_thread.rs`: let session history/fork methods cancel summary work; adapt the test-only unkeyed replacement helper.
- `src/app/state_transcript.rs`: remove the unused app command-override getter after its caller moves into the session.
- `src/app/tool_command_view.rs`: keep small app adapters for labels and source paths; move plain JSON/command work to the session.
- `src/app/tool_output_items.rs`: delegate tool facts and redraw the returned entry; retain the existing streamed-output view adapter.
- `src/app/tool_output_state.rs`: retain folding, separate shrink/grow thresholds, row choices, and DisplayMessage; remove only the plain diff summary.
- `src/app/tool_summary.rs`: import the same diff summary from its terminal-free owner.
- `src/app/transcript_diff.rs`: import the same file splitter from its terminal-free owner.
- `src/app/transcript_diff_source.rs`: retain both syntax highlighters; move only the plain splitter.
- `src/app/transcript_render.rs`: remove the renderer re-export of the already-moved read parser/formatter.
- `src/app/transcript_source.rs`: retain syntax colors, source rows, wrapping, and row counts; move source-path inference.
- `src/app/view_state.rs`: own only the unsent summary checkbox value.
- `src/session/mod.rs`: register the plain helper owners and the two new test modules.
- `src/session/session_update_tests.rs`: pass an explicit label preference to existing session fixtures.
- `src/app/shell_command.rs` → `src/session/shell_command.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/shell_directory.rs` → `src/session/shell_directory.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/shell_read.rs` → `src/session/shell_read.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/shell_search.rs` → `src/session/shell_search.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/shell_words.rs` → `src/session/shell_words.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/session/state.rs`: own SummaryState privately within the session and accept the label preference explicitly.
- `src/session/summary_binding_tests.rs`: add five terminal-free tests for both result guards, replay rules, cache entry spelling, retirement/ineligibility, and same-id replacement.
- `src/app/summary_request.rs` → `src/session/summary_request.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/session/summary_state.rs`: own accepted preference, cwd, bindings, original/generated labels, queue, service, replay flag, and revision; expose narrow commands/accessors.
- `src/session/summary_updates.rs`: observe commands, preserve disk keys and binding revisions, run the worker, apply results/settings, and return changed EntryIds.
- `src/session/thread.rs`: cancel summary work and clear bindings at accepted history/fork replacement points.
- `src/session/tool_command_view.rs`: own the existing directory parser result and plain label/source adapters without AppState or terminal imports.
- `src/app/tool_diff.rs` → `src/session/tool_diff.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/session/tool_diff_summary.rs`: own the unchanged diff counts and file-splitting parser.
- `src/app/tool_format.rs` → `src/session/tool_format.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/tool_format_command.rs` → `src/session/tool_format_command.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/session/tool_label_parity_tests.rs`: compare all helper outputs with 95 fixed cases captured from the starting commit.
- `src/session/tool_output_items.rs`: own literal/generated labels, failure facts, and the unchanged fallback chain; return the same dirty entry.
- `src/app/tool_shell.rs` → `src/session/tool_shell.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/app/tool_shell_command_summaries.rs` → `src/session/tool_shell_command_summaries.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers.
- `src/session/tool_source.rs`: own the unchanged successful-read source-path classifier.
- `src/app/tools.rs` → `src/session/tools.rs`: move the existing plain helper and its dependencies; adjust visibility/imports, retain its algorithms, and document public helpers. Extract exit-detail assembly into a small helper to keep functions under 60 lines.
- `src/session/transcript_commands.rs`: keep the label setter used only by fixtures behind cfg(test), avoiding a new production warning.
- `src/session/turn_boundary_tests.rs`: pass an explicit label preference to existing session fixtures.
- `src/tests/entry_identity_tests.rs`: set the test label preference through a session method; retain all assertions.
- `src/tests/fixtures/tool_labels_previous.json`: store the exact previous-commit inputs and expected plain helper outputs.
- `src/tests/fixtures/tool_summaries_previous.sqlite3`: store a source-label hit and command-only hit written by the previous commit's real Store writer.
- `src/tests/head_entry_reference/transcript.rs`: import the same plain read helpers directly from the session.
- `src/tests/history_conversation_tests.rs`: adapt only test preference setup to the session owner.
- `src/tests/idless_item_tests.rs`: adapt only test preference setup to the session owner.
- `src/tests/item_source_backend_tests.rs`: adapt only test preference setup to the session owner.
- `src/tests/summary_tests.rs`: adapt existing settings tests and add an isolated real-worker cancellation/cache workflow with a gated Python provider.
- `src/tests/tool_command_view_tests.rs`: adapt cwd/preference reads and checkbox writes to their respective owners; retain existing expected values.
- `src/tests/ui_render_tests/source_highlight.rs`: import source-path inference from the session; retain all color and row assertions.

**Parity ledger**

The request calls this the “part 5b” table. I did not move or split a 5b method here. Prompt recording still trims empty text, deduplicates only an already-recorded EntryId, and resets navigation only on its existing result. Native positions still deduplicate exact turn/item keys, including hidden user records. Rewind restore still clears navigation, while clearing rewind mode alone retains its navigation cursor/draft. The existing 5b tests passed unchanged apart from explicit constructor/setup values. The ledger below covers the methods moved or split by 5c, as narrow-screen lists.

- `SummaryState::default` → `SummaryState::new(enabled)` plus app startup/view construction: app loads the same saved preference once; session starts with empty cwd/maps/queue, no worker, replay=false, revision=0; view starts with the same checkbox value.
- `clear` → session summary state, called by session history/fork methods: cancel the existing worker first, clear bindings and pending ids, retain service/cwd/preference/revision. The old app replacement helper still clears before test-only unkeyed adoption.
- `retain_items` → session state, test-only as before: keep bindings whose EntryIds still resolve, then retain pending ids with bindings. Do not cancel unrelated retained jobs.
- `flush` → session state: disabled or missing service returns immediately; try the front request, stop on a full channel, otherwise pop and continue in arrival order. No blocking send or transcript scan was added.
- `label` → session state: disabled, absent binding, or absent generated label returns None; clone only the small label string, as before.
- `with_historical_tools` → app closure with session replay setter: save old replay flag, set true, execute the closure, restore old flag, return its result. Nested replay and the existing lack of panic restoration remain unchanged.
- `observe_summary_item` → session observer plus app native-scope/redraw adapter: require item id, actual command, mapped current entry, non-diff kind, and ToolCall/ToolOutput role. Missing summary input removes binding/pending work and restores a literal description only when a binding existed. Equal current key returns immediately. Otherwise remove the old binding/pending id, describe when replacing or metadata is absent, advance the checked revision, bind the request, and flush.
- `summary_cache_entry` → session observer, with plain native turn/part supplied by the app: preserve `thread:item`, or `thread:codex:[turn,item,part]` with the exact existing JSON spelling. Never put EntryId or binding revision into the disk key.
- `bind_summary_request` → session observer: save existing tool summary or `run {command}` fallback, clear generated label, append the EntryId to pending, and try the worker queue.
- `configure_summary_worker` → session: spawn with the accepted enabled flag, replace the handle, flush pending work.
- `handle_summary_event` → session plus app redraw adapter: apply only Ready(key,binding,Some(label)); ignore Ready(...,None). Show a failure only when enabled and its binding is current, or its binding string is empty. Keep the exact failure text and flush pending work after handling the event.
- `apply_summary` → session: require BOTH the stored summary key and live binding string; retain the same directory suffix; save the matching label even when disabled, but do not write the entry then. Missing/retired entries and diff entries are not changed. Return only the EntryIds whose tool metadata changed; app dirties those same rows before it handles another event.
- `apply_summary_setting` → session plus view checkbox adapter: accept the checkbox only on apply, call the existing worker switch, set request.generate=false when disabling, restore original labels when disabled or when no generated label exists, preserve bodies/failure flags, and flush. Re-enabling does not invent a new generation grant.
- `format_tool_item`, `tool_command_summary`, `tool_source_path` → session plain command owner: retain the same safe-directory checks, relative-directory authority, raw-command fallback, command override precedence, and plain JSON copy. The app still supplies the native key; syntax colors and DisplayMessage stay in the view.
- `describe_tool_output` / `tool_failed` → session: missing row returns no change; prefer generated label, then command classification, then tool-call format, then `Tool output`; diff entries use diff counts; strip terminal controls and retain the first line. Keep exit=1 diff success, error flags, and failed/declined status rules. Dirty the same entry, with the same incremental redraw log.
- `source_path_from_item` → session tool source: retain all exit/error/stderr guards, Read path-field precedence, single read action, multi-action rejection, and single-shell-read fallback. No source file is opened by this classifier.
- `diff_summary` / `split_file_diffs` → session diff helpers: retain exact header exclusions, file counting, explicit-path/new-path/old-path fallback, `/dev/null` behavior, and empty-input splitter result. No wrapping or row count moved.
- Read summary parsing already lived in `src/session/read_summary.rs`; remove only the renderer bridge. Preserve whitespace trimming, bracketed directory identity, malformed count rejection, minimum count one, and the count suffix only when count > 1.

For every moved plain helper, the input/expected-output ledger is `src/tests/fixtures/tool_labels_previous.json`, executed by `src/session/tool_label_parity_tests.rs`. Its 95 rows cover shell display/analysis; read paths, ranges, pipes, search operands, words, assignments and program names; literal shell/directory parsing; SSH parsing, quoting, path shortening and terminal-control stripping; JSON path/integer/text extraction, command/action/name/reason/description/output parsing, argument decoding and raw function conversion; plain tool names/icons/input brackets and call/output formatting; source inference, directory labels, summary input keys and eligibility; read-summary parsing/formatting; diff extraction/counts/splitting. Internal helpers are exercised through those public callers. The expected values were produced by the starting commit, rather than recomputed by the moved implementations.

**Tests and comparisons**

- Five new session tests enumerate independent key/binding matches, live/history/disabled/replay grants, exact persisted entry keys, retired entries/ineligible commands, and replacement that preserves EntryId but must discard the old binding.
- The fixed helper test compares 95 input cases with the previous commit's outputs. Existing read grouping, hidden read shells, summary-row rendering, directory labels, source syntax colors, diff counts, failures and original-label fallback tests retain their expected values.
- The isolated worker test waits for a provider receipt, replaces history while the provider blocks, and requires its socket to close from cancellation. It then receives the source-cache hit, inline-cache hit, and historical miss in order and checks final labels/bodies and absence of a saved cancelled result. The worker uses a private child-process XDG home, not global test-process environment changes. The cache fixture was written by the starting commit's actual `Store::save` (`write-cache.rs`, `cache-previous.log`). `src/summaries/` is byte-for-byte unchanged.
- The strict live/history harness sampled 3,000 streams with seed **516053** from `target/m4-fix/streams.jsonl`: 1,025 Claude, 994 Grok, 981 Codex. All **54,012 checkpoints**, session facts, accepted-prompt facts, backend calls, redraw calls, and rendered/styled lines match the starting commit exactly; neither side panicked. See `sample-manifest.json`, `comparison.json`, `hashes.json`, and the six paired output files.
- I also ran the same 3,000 streams through the saved pre-plan oracle for `a59221c` and the previous/current trees using its older comparable fact/screen shape. All three had zero panics. Exactly **278 streams already differed between pre-plan and the starting commit**, and current has **zero additional differences** from the starting commit. See `preplan-comparison.json` and `preplan-difference-fields.json`. I checked the saved pre-plan executable against its recorded SHA-256 (`preplan-oracle-provenance.json`). The strict comparison above additionally includes fields the older oracle cannot name, such as EntryIds and split prompt records. As in the earlier harness, it normalizes independently generated EntryId namespaces while comparing their identity relationships.
- The mutation harness deliberately disables label generation, so it does not prove worker cancellation, cache hits/misses, or the delayed-result guards. The enumerated tests and gated worker workflow cover those paths.

**Checks and speed**

- `cargo build`: pass.
- `cargo build --release`: pass, default features, Rust 1.100.0.
- `cargo fmt --check`: pass.
- `bash scripts/check-rust-style.sh --all`: pass; no changed Rust file exceeds 400 lines or changed function exceeds 60 lines (`function-sizes.json`).
- `cargo test` twice: both pass, **890 passed, 3 ignored**. Test bodies took 7.75 seconds during the concurrent audit build, then 5.98 seconds. The terminal script's Rust run also passed.
- `bash scripts/test.sh` once: pass, **174 terminal workflows**. The workflows check screens and backend requests, including summary updates, cached history, source colors, and quota text.
- The only compiler warning in the final production/test builds is the pre-existing deprecated atomic call in `protocol_writer.rs`.

I ran seven alternating old/new release-binary pairs for both recorded perf cases at 160×48. I repeated the timing run after all compilers, terminal checks and mutation harnesses stopped; use `quiet-perf-medians.json`, not the earlier loaded measurements. Median milliseconds, previous/current:

- synthetic: replay 47.43/47.11; full layout 14.90/14.68; full draw 3.60/3.46; scroll p50 0.71/0.70; typing p50 0.60/0.60; working p50 0.59/0.60; append p50 0.60/0.60.
- history: replay 1.06/1.11; full layout 0.01/0.01; full draw 0.48/0.51; scroll p50 0.43/0.47; typing p50 0.36/0.37; working p50 0.36/0.37; append p50 0.39/0.39.

The synthetic case has the same 6,001 entries and 39,347 rendered lines. Every quiet median stays within the plan's 10% limit. The code adds no transcript-length work per frame, row, or delta; the existing binding scan still runs only when applying labels/settings. The paired redraw logs match exactly.

**Small edits outside Milestone 5's named app paths**

- `src/app/item_history_draft.rs`: change one import so a future core does not import the read parser through a renderer.
- `src/app/perf_synthetic.rs`: change one import after removing that renderer export.
- `src/app/models.rs`: make an unused production re-export test-only after its production consumer moves.
- `src/app/state_thread.rs`: remove the two app-side summary clears now owned by session replacement; adapt the test-only clear call.
- `src/app/state_transcript.rs`: remove the unused four-line app getter after moving its caller.
- `src/app/tool_summary.rs`: change one import to the plain diff helper.
- `src/app/transcript_diff.rs`: change one import to the plain file splitter and retain the styled highlighter import.

P2 additionally edits `src/protocol.rs` and `src/tests.rs` solely to register the repaired private-state test in its owning module. Those are preliminary test changes, not part of `m5c.patch`. P1's additions in backend setup files are also test-only. All remaining edits are in the requested session/helper/view paths or affected tests. I did not change 5a runtime settings, 5b prompt/history algorithms, or 5d quota/rate-limit arithmetic.

**Discoveries and decisions for the plan**

- The session now cancels label work at its own accepted history/fork methods, so app callers cannot forget that step. Revisions remain monotonic across clear, and the worker/service handle survives replacement.
- The existing cache already separates its disk entry key from the in-memory result binding. Keeping both strings unchanged required no worker/cache/schema edit.
- The app must load saved UI preferences and pass the accepted boolean to the session constructor. The unsent settings checkbox remains entirely in the view.
- Carlos still permits matching labels to arrive while labels are disabled and remembers them for a later enable. It still suppresses generation grants for replay and does not add generation grants merely on re-enable.
- Carlos still lets a running job for a changed/ineligible command finish; its stale binding prevents an entry write. I did not add individual-job cancellation or a stronger global-error generation fence. Those would be separate daemon decisions.
- A caller error still does not promise that the independent event channel has already received its closure notice. The repaired tests now respect that existing contract.
- Low-level transcript fixture/store methods remain internal building blocks; future daemon callers should use the session history/fork methods that also retire label bindings. I did not broaden every raw store write into a new cancellation guarantee.

No requested work remains unfinished. I left the working tree uncommitted and did not install the executable.
