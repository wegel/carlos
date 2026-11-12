Carlos Milestone 4 report, 2026-11-12 UTC

I implemented Milestone 4 of EXECPLAN_016 on main, above HEAD 0f5f2b4. I left the changes uncommitted. SessionState now owns the transcript, native entry maps, turn facts, parsed command labels, pending approval, Ralph automatic-loop facts, thread identity/name, status and context counts. ViewState owns the terminal widgets, viewport, render caches, layout, tool-fold choices, dictation and performance counters. AppState explicitly retains six mixed children for milestones 5 through 9: summaries, input_history, runtime, claude, codex and rate_limits.

I changed no dependency, wire protocol, Rust toolchain, .agents/ file or docs/ file. I did not commit, push, stash, reset Git state or install a binary. The checkout still selects rustc 1.100.0 (0ef1bc386 2026-11-09) through its existing directory override. I removed /tmp/carlos-head-ref after restoring its temporary edits; I also removed every temporary test hook from the main source tree. All harness files, inputs, outputs and logs remain under target/m4/.

The app borrows session facts through accessors and writes through named session commands. SessionState fields are private to src/session/. No SessionState method accepts AppState, ViewState, a widget or a widget-mutating callback. No session file references crate::app, ratatui, crossterm or RefCell. I added no Deref adapter or shared mutable state. The existing read-only Transcript slice adapter remains unchanged. I did not clone the transcript to resolve a borrow conflict.

SessionUpdate carries an earliest changed EntryId, a full-replacement flag or an empty-tail flag. The app resolves the changed id once and uses the existing mark_transcript_dirty_from method. The report preserves the preceding read-summary row and the cache still rebuilds following rows whose spacing depends on that predecessor. Reasoning updates retain both original dirty calls. Replacement retains the existing full-reset call; a fork does not acquire an extra dirty-from-zero call. These methods add only fixed-size reports and constant-time id lookups, with no new transcript-length scan or per-frame copy.

The private TurnBoundary retains the old suffix offset and records the first appended EntryId. Its empty state exists even when the transcript has no rows. It does not use the allocator counter. Ralph scans exactly the prior suffix; an absent saved boundary still means the current end for Ralph. The session resolves the active reasoning row for the renderer, so production app code never reads or stores the saved boundary cursor. Test-only readers inspect it for parity checks.

Checks and measured results

- cargo build: passed, including the final cached check in 0.110 s.
- cargo build --release: passed with default features; final optimized rebuild 45.65 s.
- cargo build --release --no-default-features: passed, 44.46 s; three synthetic and three history-fixture smoke runs passed with those flags. The final target/release/carlos uses default features.
- cargo fmt --check: passed; final wall 0.398 s.
- bash scripts/check-rust-style.sh --all: passed; final wall 1.827 s.
- git diff --check: passed.
- Three consecutive final cargo test runs: 3.874 s, 3.878 s, 3.864 s, each 828 passed, 2 ignored, zero failures. All remain below 8 s. I added six permanent tests and weakened or deleted no existing test.
- bash scripts/test.sh: passed, 113.325 s overall, 171 terminal workflows in 79.134 s, 828 Rust tests passed/2 ignored, both 59-test controller runs, recorder and package smoke passed. The exact reload-then-rewind screen/native-target workflow passed. Artifacts: target/ui-artifacts/run-nxYEaK/.
- The compiler emits only the unchanged deprecated AtomicUsize::fetch_update warning at src/protocol_writer.rs:72. No new warning remains.

The first attempt to collect three consecutive passes hit only protocol::history_transport_tests::unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls on runs 3 and 5. Each failed run had 827 passed, 1 failed, 2 ignored, with walls 3.857 s and 3.882 s. Runs 6, 7 and 8 then passed consecutively. After restoring one direct store fixture through an exact test-only session adapter, I warmed the test executable and collected the final three passes above without failures. I preserved both attempts in test-three-before-fixture-adapter.json, before-fixture-adapter-cargo-test-*.log, test-three.json and cargo-test-*.log. I changed no test deadline or backend code.

The six added permanent tests enumerate these cases:

- turn_boundary_tests::every_short_turn_history_keeps_the_same_suffix_and_first_entry: all 16,807 length-five histories over seven steps, checking 84,035 boundaries against the former saved-offset rules.
- turn_boundary_tests::empty_and_interrupted_turns_scan_only_the_saved_ralph_suffix: 80 combinations of old markers, empty/new suffixes, start presence and interrupt state.
- session_update_tests::every_row_edit_reports_heads_earliest_dirty_entry: 1,680 combinations of predecessor text, optional new text, all seven roles, all three stored kinds and valid/missing indices.
- session_update_tests::every_stream_completion_and_replacement_reports_only_its_existing_dirty_calls: 84 stream cases across all backends/roles/completed/mapped states, plus 16 empty history replacements.
- session_update_tests::empty_tail_and_repeated_markers_preserve_incremental_reports: all 36 cuts of zero through seven rows, including repeated interruption/compaction notices.
- app/state_transcript_checks::grouped_shell_updates_dirty_the_preceding_label_and_keep_following_spacing: 32 combinations of width, theme, hidden rewind row and changed role, each checking three successive edits against a fresh full layout, with text and styles compared.

Mutation parity against unchanged HEAD

I created the requested detached worktree with git worktree add --detach /tmp/carlos-head-ref HEAD. Temporary Rust tests in both trees handled the same JSONL corpus through input_dispatch::process_event and the full app handlers. Claude native fixture frames passed through the production conversation guard and reader normalizer, then actual ClaudeClient::handle_event on the calling thread. Grok ACP fixture frames passed through the real Grok translator before the app dispatcher. Codex notifications came from the earlier captured/fixed history corpus and common streamed-item fixtures. The client mock supplies deterministic backend responses and records calls; it does not replace app transcript/turn/approval handling.

The generator used seed 0x164004 and produced 24,000 streams: 8,000 Codex, 8,000 Claude and 8,000 Grok. It loaded 3,000 Codex fixture streams, 12,000 native Claude streams and 4,096 Grok ACP fixtures from the earlier milestone evidence; the copied inputs live under target/m4/fixtures/. Mutations duplicate, drop, reorder and corrupt fields, insert streamed/common events and truncate streams. Every family also receives submit, interrupt, rewind, approval answers, Ralph toggle, selection and/or full history-replacement actions. The corpus also includes both-backend-enabled flag cases. streams.jsonl and corpus.json retain the exact inputs and mutation counts.

Each stream prints canonical JSON for every stored transcript fact, normalized Carlos entry-id relationships, native keys, turn statuses, active turn, pending/queued approval facts, Ralph state/queue/deadlines-as-presence, context counts, status and thread metadata, plus the exact backend calls and the complete ordered dirty-from trace. The test actually draws a terminal at 160x48 and 60x20 after every event, and captures all cached transcript lines with text, role, wrap/separator/cell facts and every segment style. It stores a hash of that full canonical state and both rendered views after every event, then prints the full final JSON. Only random id namespaces and physical Instant values are normalized; no behavior adapter or accepted-difference rule is used.

Final result: 24,000/24,000 exact matches; zero differences in facts, backend calls, dirty traces, rendered lines/styles or intermediate snapshot hashes. Both trees completed 22,756 streams without panic. Both trees hit the same pre-existing "no entry found for key" panic in notification_tools::complete_tool_item on the other 1,244 malformed/reordered streams; the temporary harness catches that panic, records it and the partial facts/dirty trace, then moves to the next stream. It does not conceal those cases as successes. I did not change this unrelated panic because Milestone 4 requires identical behavior. The final comparison after moving the active-reasoning lookup also matched all 24,000 streams. Comparison files: comparison.json, differences.json, head.jsonl, current.jsonl, current-final-shards.json. The first complete run also matched all 24,000; I kept the partial-run and shard evidence.

The temporary hooks, native replay export and ignored audit test are absent from src/. The ignored-test count returned to the original two. The reference worktree had a clean status before removal. The saved native-replay.rs, head-audit.rs/current-audit.rs, generator, shard runners, source backups and copied HEAD source remain under target/m4/. This corpus establishes the stated parity for its fixed fixtures and seed; it cannot enumerate every possible native input or thread schedule. No thread boundary moved in Milestone 4.

Speed, seven alternating pairs, no build or test running alongside them

I ran /tmp/carlos-base/target/release/carlos (old, worktree a59221c) then ./target/release/carlos (new) seven times each with exactly perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48. All fourteen reports contain 6,001 messages, 39,347 rendered lines and 6,000 relevant items. Both executables use the existing Rust 1.100.0/default-feature baseline. I did not replace or rebuild the supplied old executable.

- replay_elapsed_ms: 49.33 -> 47.71 ms (-3.28%).
- full_layout: 14.88 -> 14.37 ms (-3.43%).
- full_draw: 3.51 -> 3.49 ms (-0.57%).
- scroll_draw p50: 0.70 -> 0.69 ms (-1.43%).
- typing_draw p50: 0.59 -> 0.59 ms (+0.00%).
- working_draw p50: 0.59 -> 0.60 ms (+1.69%).
- append_total p50: 0.59 -> 0.60 ms (+1.69%).

Every requested median passes the 5% gate; the largest increase is 1.69%. Median p95 values also remain within 5%: replay apply 15.17 -> 15.54 ms, scroll 0.81 -> 0.78, typing 0.60 -> 0.60, working 0.60 -> 0.60, append 0.63 -> 0.63. Three final default-feature history-fixture runs also passed, replay 1.17, 1.27 and 1.08 ms (median 1.17), with the same expected five messages/nine lines/four replay items. Raw reports and all samples live in perf-pairs.json, perf-p95.json, perf-{old,new}-*.txt and perf-history-*.txt. I discarded an earlier loop that overlapped a test-executable rebuild; its raw data remains under perf-under-build/. I make no memory claim from the benchmark's known broken zero-RSS fields.

A session command now finishes its private fact writes before the app consumes its redraw report. The old append helpers dirtied the view before their caller bound a native key or finalized Ralph status; the new helper binds/finishes those facts before returning the same dirty entry. The cache dirty method reads only the transcript length and row position, and no draw, backend control, callback or event handler runs between these steps. The fork command likewise groups boundary/context clearing with the other session resets before the app reconstructs its cache/history helpers. The audit table records the old order; the cross-tree traces and per-event rendered states verify the observable result.

Files outside the Milestone 4 allowed list

- src/app/approval_state.rs: Replace the app-owned plain approval types with narrow session reexports; otherwise SessionState would depend on crate::app.
- src/app/backend_setup.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.
- src/app/backend_setup_grok.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.
- src/app/backend_setup_launch.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.
- src/app/context_usage.rs: Move only ContextUsage into the session and reexport it here; leave parsing and label formatting in place.
- src/app/layout_state.rs: Change the content_x read to view.layout; no layout rule changed.
- src/app/mobile_mouse.rs: Change viewport field paths to view.viewport; retain every mouse rule and calculation.
- src/app/native_questions.rs: Read pending approval through the immutable session accessor; retain modal/editor selection rules.
- src/app/ralph.rs: Reexport the test-facing plain Ralph types/constants after moving the actual automatic-loop facts/helpers to the session.
- src/app/ralph_runtime_state.rs: Reexport QueuedTurnInput after moving the runtime owner and its implementation into the session.
- src/app/ralph_turn_queue.rs: Delete the now-obsolete app-side queue implementation after moving its complete body under the session owner.
- src/app/task_badge.rs: Read the root thread id through the session accessor; retain every ownership/count rule.
- src/app/transcript_render.rs: Reexport the three unchanged plain read-label helpers from session/read_summary; Milestone 4 read grouping cannot import a renderer. This is the minimum helper extraction pulled forward from Milestone 5.

I made these thirteen minimum dependency/access-path edits because removing the old AppState fields makes those existing callers fail to compile. No other source path outside the allowed list changed. /tmp/carlos-m4-report.md is the explicitly requested external report; target/m4/ contains ignored verification artifacts.

Style limits and existing caller debt

The repository style script passes, all changed/new files have at most 400 lines, and every new or moved function has at most 60 lines. I also used a string/comment-aware function scan against HEAD. Nine existing caller functions already exceeded 60 lines before this task; their bodies received only owner access-path edits. I retained them to honor the instruction that callers outside the named transcript/turn methods change only access paths. This is an explicit limit of the strict 60-line claim, rather than a claim that all existing callers now meet it. Details are in function-sizes-with-head.json:

- src/app/codex_goal_text.rs::body: HEAD 117 lines, current 117 lines.
- src/app/codex_panel_events.rs::event: HEAD 82 lines, current 85 lines.
- src/app/codex_panel_receive.rs::stop_reply: HEAD 71 lines, current 71 lines.
- src/app/input.rs::run_one_loop_iteration: HEAD 64 lines, current 64 lines.
- src/app/input_dispatch.rs::dispatch_action: HEAD 62 lines, current 62 lines.
- src/app/input_events_keys.rs::handle_key_event: HEAD 67 lines, current 67 lines.
- src/app/input_events_model_settings.rs::save_settings: HEAD 68 lines, current 68 lines.
- src/app/mouse_events.rs::handle_mouse_event: HEAD 62 lines, current 62 lines.
- src/tests/codex_goal_lifecycle_tests.rs::visible_question_paste_and_mouse_take_priority_over_a_hidden_goal_editor: HEAD 70 lines, current 70 lines.

The codex_panel_events::event increase from 82 to 85 lines comes from rustfmt wrapping the longer accessor path; its rules and order are unchanged. The new clipboard companion keeps input_events_keys.rs below 400 lines without changing its keyboard behavior. I added no style suppression or exception marker. If the 60-line limit is intended to cover every pre-existing caller touched by the mechanical pass, these nine functions remain outside that limit; splitting their unrelated handlers would exceed the access-path-only scope.

Discoveries and decisions for the plan's living sections

- Progress: Milestone 4 now has the two owners, private session facts, transcript/item/source/turn writes under src/session, exact incremental reports, and enumeration plus cross-tree evidence. Leave milestones 5 through 11 unchecked.
- Surprises & Discoveries: malformed/reordered tool fixtures can panic on unchanged HEAD; this corpus records 1,244 matching cases. The Codex 20 ms deadline flake recurred twice with no source change. Nine existing caller functions exceed the stated function cap.
- Decision Log: use a private offset plus first-entry id for an empty-capable TurnBoundary; expose only the computed active reasoning row to the renderer. Keep two separate reasoning dirty reports. Give test-only suffix removal an explicit dirty-tail report. Extract only the plain read-label/Ralph/approval/context dependencies needed for the M4 terminal boundary.
- Outcomes & Retrospective: 24,000 exact handler/render/dirty matches, six enumerating regression tests, 828 Rust tests/2 ignored, 171 terminal workflows, no requested speed median more than 5% slower. Keep the six mixed AppState children for their named later milestones. I did not edit the plan itself, as requested.

Parity audit table

I audited every moved or split method below against the saved HEAD source. The table also identifies retained app guards and mixed bridges, so a reader can locate every old rule after the split. I separately compared the exact method bodies for the plain dependencies after whitespace normalization: all 50 copied bodies match HEAD, including approval native answer shapes, Ralph marker/queue rules, read-label parsing and reasoning text normalization. Evidence: parity-audit.json, parity-audit.md, unchanged-bodies.json, head-source/.

| Old method | HEAD guards, defaults, order, and dirty calls | New location |
|---|---|---|
| `state::new` | Empty thread/name/active/status/context, fresh transcript allocator, empty maps/queue/approval; identical widget/default constructors. Preferences remain app-side. | session/state::new; app/view_state::new; app/state::new retains the six explicitly temporary mixed children |
| `state::configure_ralph_options` | Pass cwd and all five supplied path/marker choices unchanged; no loading during construction. | session/turns::configure_ralph_options; app adapter retains its old result |
| `state::enable_ralph_mode` | Enable first; use returned path-bearing status. Test-only as before. | session/turns::enable_ralph_mode; app adapter retains its old result |
| `state::disable_ralph_mode` | Disable runtime first, then set its returned status. | session/turns::disable_ralph_mode; app adapter retains its old result |
| `state::queue_ralph_continuation` | Delegate nonblank filtering and the 700 ms delay to the same queue owner. | session/turns::queue_ralph_continuation; app adapter retains its old result |
| `state::queue_turn_input` | Use record_input_history=true; test-only as before. | session/turns::queue_turn_input; app adapter retains its old result |
| `state::queue_turn_input_with_history` | Keep the caller-provided history choice and human origin. | session/turns::queue_turn_input_with_history; app adapter retains its old result |
| `state::has_ready_queued_turn_input` | Check ready queue before the delayed continuation deadline. | session/turns::has_ready_queued_turn_input; app adapter retains its old result |
| `state::promote_ready_continuation` | Promote only a due continuation; append behind human text. | session/turns::promote_ready_continuation; app adapter retains its old result |
| `state::has_pending_ralph_continuation` | Check both the delayed entry and queued keeps_ralph_pending entries. | session/turns::has_pending_ralph_continuation; app adapter retains its old result |
| `state::pending_ralph_continuation_wait` | Return no delay when ready queue is nonempty; omit zero wait. | session/turns::pending_ralph_continuation_wait; app adapter retains its old result |
| `state::dequeue_turn_input` | Promote due continuation before popping one ready entry. | session/turns::dequeue_turn_input; app adapter retains its old result |
| `state::ralph_enabled` | Read current.is_some; no transcript work. | session/turns::ralph_enabled; app adapter retains its old result |
| `state::queued_turn_inputs_is_empty` | Read ready queue only; keep test-only visibility. | session/turns::queued_turn_inputs_is_empty; app adapter retains its old result |
| `state::ralph_toggle_pending` | Read the pending toggle bit; keep test-only adapter. | session/turns::ralph_toggle_pending; app adapter retains its old result |
| `state::ralph_pending_continuation_deadline` | Read the stored deadline without changing it; keep test-only adapter. | session/turns::ralph_pending_continuation_deadline; app adapter retains its old result |
| `state::ralph_waiting_for_user` | Missing current Ralph means false; otherwise read its waiting flag. | session/turns::ralph_waiting_for_user; app adapter retains its old result |
| `state::set_status` | Replace the string verbatim, including empty strings; no redraw or backend call. | session/state_commands::set_status |
| `state::set_pending_approval` | Same-request existing entry id wins, then supplied id, then allocate one card id; set title-bearing status before replacing pending. No transcript row or dirty call. | session/state_commands::set_pending_approval |
| `state::clear_pending_approval` | Retire an owned Codex callback and return; otherwise remove the matching Claude draft, then promote exactly one queued question. | app/state::clear_pending_approval retains queue/editor decisions; session/state_commands::{take_pending_approval,replace_pending_approval} own writes |
| `state::append_context_compacted_marker` | Ignore only an adjacent System row with exactly ↻ Context compacted; otherwise append one System row and dirty that new entry once. | session/turns::append_context_compacted_marker returns SessionUpdate; app applies it once |
| `state::append_turn_interrupted_marker` | Ignore only an adjacent System row with exactly Turn interrupted; otherwise append one System row and dirty that new entry once. | session/turns::append_turn_interrupted_marker returns SessionUpdate; app applies it once |
| `state::mark_turn_started` | Save the transcript end even when it is empty. Do not allocate an entry or alter allocator counters. | session/turns::mark_turn_started stores TurnBoundary { offset, first: None }; append observes its first entry |
| `state::record_turn_started` | Return false for the exact active id or any terminal status; otherwise set active, insert InProgress, then mark boundary, then return true. | session/turns::record_turn_started |
| `state::record_turn_completed` | Ignore nonterminal status; record terminal status even for a nonactive id, then return false unless ids match; clear only the matching active id. Do not consume the Ralph boundary. | session/turns::record_turn_completed |
| `state::record_historical_turn_status` | Overwrite the named historical status without changing active turn or boundary. | session/turns::record_historical_turn_status |
| `state::maybe_disable_ralph_on_blocked_marker` | Absent boundary means empty suffix at current end. No outcome returns without writes/dirtying. Append a nonduplicate notice first, disable second, outcome status last. | session/turns::maybe_disable_ralph_on_blocked_marker; exactly one optional dirty report consumed by app |
| `state::handle_ralph_turn_completed` | Take boundary before scanning; absent boundary means current end. Preserve the empty/interrupted-empty suffix. Notice if not adjacent duplicate, then queue continuation, then disable, then final outcome status. | session/turns::handle_ralph_turn_completed; runtime marker rules unchanged |
| `state::{rewind_mode,input_history_message_indices,input_history_len,set_rewind_selection_for_test}` | Stay app-side; history positions resolve EntryId at the read boundary. Same defaults, optional indices, navigation, and test-only guards. | same app/state methods with read accessors |
| `state::{enable_perf_metrics,perf_report}` | Stay app-side; create counters only on explicit enable, return no report when disabled. | same app/state methods through view.perf |
| `state_items::item_key` | Codex wins over Claude over Grok. Claude prompt prefix remains typed; Codex alone qualifies turn; preserve conversation and part. No id allocation. | same app/state_items method reads session entry_conversation |
| `state_items::with_item_part` | Save old native part, run closure with exact native item, restore old part, return closure result. | same app/state_items method; no session callback accepts widgets |
| `state_items::item_message_index` | Resolve native key to id, then current position; missing key/id returns None. | session/items::item_message_index; app builds one concrete native key and consumes any returned report |
| `state_items::has_item_mapping` | True only if the key currently resolves. | session/items::has_item_mapping; app builds one concrete native key and consumes any returned report |
| `state_items::put_item_mapping` | Activate the addressed entry before binding its key. | session/items::put_item_mapping; app builds one concrete native key and consumes any returned report |
| `state_items::mark_item_completed` | Mark entry completion before inserting the exact scoped key into completed set. | session/items::mark_item_completed; app builds one concrete native key and consumes any returned report |
| `state_items::is_item_completed` | Read the scoped native completion key, including unmapped completed keys. | session/items::is_item_completed; app builds one concrete native key and consumes any returned report |
| `state_items::ensure_item_placeholder` | Existing mapping returns false with no dirty call; otherwise append an empty row, then bind it, then return true. | session/items::ensure_item_placeholder; app builds one concrete native key and consumes any returned report |
| `state_items::update_mapped_message` | Missing mapping returns false with no dirty call; existing mapping uses the same row-update/read-group rules and returns true. | session/items::update_mapped_message; app builds one concrete native key and consumes any returned report |
| `state_items::upsert_mapped_message` | Try update first, preserving clones/default path rules. Existing row returns false; new Diff uses diff append, other kinds use plain append then set path; bind after append; return true. | session/items::upsert_mapped_message; app builds one concrete native key and consumes any returned report |
| `state_items::upsert_agent_delta` | Use Assistant role without replacing an existing plain role. Completed keys do nothing. | session/items::upsert_agent_delta; app builds one concrete native key and consumes any returned report |
| `state_items::upsert_command_output_delta` | Use ToolOutput and replace the existing role; completed keys do nothing. | session/items::upsert_command_output_delta; app builds one concrete native key and consumes any returned report |
| `state_items::upsert_text_delta` | Completed key returns before writes/dirtying. Existing row appends via unchanged store rules and dirties that index once. Missing mapping appends then binds, preserving read-group dirty predecessor. | session/items::upsert_text_delta; app builds one concrete native key and consumes any returned report |
| `state_items::upsert_reasoning_summary_delta` | Completed guard first, ordinary Reasoning append next; preserve its first dirty call. Missing mapping then returns. Normalize bold headings, preserve role/kind/path/tool facts, dirty same row a second time even if text is unchanged. | app adapter keeps guard/order; session/item_updates::{upsert_reasoning_delta,normalize_reasoning_at} own writes; two reports stay separate |
| `state_transcript::append_message` | Build EntryDraft::plain, then follow the same draft path. | session/transcript_updates::append_message |
| `state_transcript::append_draft_message` | Allocate first; coalesce afterward; dirty preceding read row if grouped, otherwise the appended row; return appended position, including hidden shells. | session/transcript_updates::append_draft_message |
| `state_transcript::append_diff_message` | Allocate Diff with supplied role/path; never coalesce; dirty only appended index. | session/transcript_updates::append_diff_message |
| `state_transcript::command_override` | Return cloned label for the exact scoped key or None. | session/item_updates::command_override |
| `state_transcript::upsert_turn_diff` | Ignore whitespace-only input. Existing same text and Diff kind does not dirty. Otherwise replace Role::ToolOutput/Diff/path=None and dirty existing row; missing id appends then binds aggregate turn key. | session/transcript_updates::upsert_turn_diff |
| `state_transcript::set_command_override` | Store scoped override first. Existing tool facts change summary only, retaining generated/failure/body/path, and dirty once; otherwise update mapped ToolCall/Plain/path=None; missing mapping does not append. | session/item_updates::set_command_override |
| `state_transcript::coalesce_successive_read_summary_at` | Reject index 0/out-of-range, non-ToolCall, non-Plain, multiline, empty, malformed labels, or unequal paths. Walk preceding empty tool shells exactly as before; add counts on preceding row with its path; clear current text/path while retaining both ids and tool facts; return predecessor. | session/transcript_updates::coalesce_successive_read_summary_at |
| `state_transcript::update_message_at_index` | Missing index does nothing. Store update first; only Plain ToolCall invokes grouping; report earliest changed row, otherwise exactly the addressed index. | session/transcript_updates::update_message_at_index |
| `state_transcript::read_summary_at` | Require plain single-line nonempty ToolCall, then parse label; retain count/path defaults. | session/transcript_updates::read_summary_at |
| `state_transcript::scan_back_past_empty_tool_shells` | Start at saturating predecessor; skip only empty Plain ToolCall shells, stop at 0 or first different row. No new scan. | session/transcript_updates::scan_back_past_empty_tool_shells |
| `state_transcript::normalize_reasoning_summary_stream` | Preserve bold-heading spacing, newline insertion, incomplete bold suffix, Unicode copying, and whitespace-before-next-bold rules. | session/reasoning::normalize_reasoning_summary_stream |
| `state_transcript::skip_whitespace_before_bold` | Skip only whitespace preceding another bold heading; leave ordinary intervening prose untouched. | session/reasoning::skip_whitespace_before_bold |
| `state_transcript::{mark_transcript_dirty,mark_transcript_dirty_from}` | Full reset still means dirty-from 0. Incremental path still clamps at transcript length and keeps the earliest index in the same existing cache method. Predecessor-dependent spacing is rebuilt by the unchanged cache over that suffix. | same app methods through view.render_cache; apply_session_update resolves one EntryId in O(1) |
| `state_transcript::{ensure_rendered_lines,sync_auto_follow_bottom,rendered_line_count,rendered_line_at,ensure_rendered_range_materialized,snapshot_rendered_lines,selected_text,RenderedLineSource::len,RenderedLineSource::get}` | Remain app-side with the same hidden-user, width/compact/theme, selection clearing, id-bearing scroll anchor, materialization and selection rules. No renderer algorithm or transcript clone added. | same app methods with session read accessors and view fields |
| `state_thread::replace_history_draft` | Clear summary bindings first; adopt checked entries then clear completion/status/command maps; preserve native-history enabled bit; clear reports; restore turn statuses then records in order; reset cache and viewport; active turn gets boundary at new end, otherwise no boundary; finally dirty-from 0 once. | session/thread::replace_history owns transcript/maps; app adapter retains mixed history/summary/backend steps; session/state_commands::restore_turn_boundary; app consumes reset report at original final step |
| `state_thread::restore_history_record` | Set temporary turn and report panel before row edits. For each edit update then optionally describe tool; prompts then summary observation; missing item id returns here. Native user records count even without visible row; classify position 0 as Plain, later as UserSteer; completion last. | same app helper preserves event order; plain row/kind writes use session/transcript_commands; prompt/editor ownership remains for M5 |
| `state_thread::reset_for_forked_thread` | Accept thread, set entry conversation, clear active and reset transcript; construct cache; clear completion/status/labels; cancel summaries; rebuild history retaining native-enabled bit; clear boundary; reset viewport; clear context; then Codex reset or clear/promote approval. No dirty-from 0 call. | session/thread::reset_for_forked_thread owns plain reset; app resets cache from reset report and retains mixed children/approval order |
| `state_thread::replace_transcript_for_test` | Cancel summary jobs, adopt unkeyed copies in SameThread mode, reset cache/viewport, dirty-from 0. Existing fixture clone preserved; no production borrow-checker clone added. | same app fixture adapter uses session/transcript_adopt_unkeyed |
| `state_input::rewind_fork_from_message_idx` | Ignore None or index beyond end; truncate before a live row, including clearing maps/overrides; selection/mouse/follow/scroll changes then retain summary/history bindings; dirty exactly old cut, including an empty tail. Test-only as before. | session/thread::truncate_at returns dirty_tail; app keeps view/binding cleanup and consumes report last |
| `state_item_sources::prepare_item_source` | No native evidence invokes reject-unproven handling and returns; all_parts count uses max(native/live parts,1); preserve each part in order. | session/item_sources::prepare_item_source; app passes concrete key plus the original Claude-enabled guard |
| `state_item_sources::attach_item_source` | Missing evidence returns; bind native source to each existing live part; stop after one when all_parts=false. | session/item_sources::attach_item_source; app passes concrete key plus the original Claude-enabled guard |
| `state_item_sources::native_source_key` | Require supplied nonempty native id; require assistant block, exact tool call/result discriminator, or user kind; nonempty supplied session overrides conversation; otherwise retain original key. | session/item_sources::native_source_key; app passes concrete key plus the original Claude-enabled guard |
| `state_item_sources::prepare_cursor` | Native source wins; retire old cursors and completion aliases; preserve completed fact on new cursor; otherwise unbind only a conflicting alias. | session/item_sources::prepare_cursor; app passes concrete key plus the original Claude-enabled guard |
| `state_item_sources::reject_unproven_cursor` | Check original claude.enabled boolean, including both-backend flag combinations; inspect at least one part; unbind only aliases with known source; remove corresponding completion keys. | session/item_sources::reject_unproven_cursor; app passes concrete key plus the original Claude-enabled guard |
| `state_item_sources::cursor_conflicts` | For unproven cursor, tool call/result and assistant roles must match source category; other kinds do not conflict. Known native source conflicts only if typed keys differ. | session/item_sources::cursor_conflicts; app passes concrete key plus the original Claude-enabled guard |
| `render_transcript::active_step_index` | No active turn returns None; absent boundary uses start 0; same active-step lookup; return only a Reasoning row. No new per-frame traversal. | session/turns::active_reasoning_step; app helper delegates; production app never reads the saved turn cursor |
| `session_name::{set_thread_id,set_thread_name}` | Changed thread resets Codex callbacks/panel/tasks/goals/epoch/queue/settings first while retaining queue mode and legacy overrides; then clear old name and set id. Name update increments revision before identical terminal-safe trimming/filtering. | same app backend/sanitizing steps; session/state_commands owns id/name assignments |
| `input_events_keys::handle_ctrl_y_copy` | Selection has priority; copy only nonempty selected text; otherwise copy last assistant answer and set identical backend-label status; always request draw. | app/input_events_clipboard::handle_ctrl_y_copy, byte-identical body except access paths; split solely to keep keyboard module under 400 lines |
| `ralph_runtime_state::new` | No current state, queue, toggle or pending continuation; empty path overrides. | session/ralph_runtime_state::new (unchanged body) |
| `ralph_runtime_state::configure_options` | Assign supplied choices verbatim. | session/ralph_runtime_state::configure_options (unchanged body) |
| `ralph_runtime_state::is_enabled` | current.is_some. | session/ralph_runtime_state::is_enabled (unchanged body) |
| `ralph_runtime_state::state` | Borrow current or None; test-only. | session/ralph_runtime_state::state (unchanged body) |
| `ralph_runtime_state::enable` | Create RalphState, clear pending toggle, return path-bearing status. | session/ralph_runtime_state::enable (unchanged body) |
| `ralph_runtime_state::disable` | Clear current, toggle, delayed continuation and ready queue; return ralph off. | session/ralph_runtime_state::disable (unchanged body) |
| `ralph_runtime_state::stop_after_failure` | Clear automatic state; retain only Human queued origins. | session/ralph_runtime_state::stop_after_failure (unchanged body) |
| `ralph_runtime_state::take_queued_human_drafts` | Drain ready queue, keep Human text in order. | session/ralph_runtime_state::take_queued_human_drafts (unchanged body) |
| `ralph_runtime_state::queue_initial_prompt` | Missing/current primed/completed returns; otherwise prime first and enqueue Automatic base prompt without history. | session/ralph_runtime_state::queue_initial_prompt (unchanged body) |
| `ralph_runtime_state::toggle_now` | Enabled disables and returns; otherwise load the same prompt/config, propagate errors without enabling, enable then queue initial prompt. | session/ralph_runtime_state::toggle_now (unchanged body) |
| `ralph_runtime_state::request_toggle` | During active turn flip pending bit and return exact pending/canceled text; otherwise toggle now. | session/ralph_runtime_state::request_toggle (unchanged body) |
| `ralph_runtime_state::apply_pending_toggle` | No pending bit returns None; otherwise clear bit before toggling and propagate error. | session/ralph_runtime_state::apply_pending_toggle (unchanged body) |
| `ralph_runtime_state::toggle_pending` | Read bit. | session/ralph_runtime_state::toggle_pending (unchanged body) |
| `ralph_runtime_state::mark_user_turn_submitted` | Clear waiting flag only when current exists. | session/ralph_runtime_state::mark_user_turn_submitted (unchanged body) |
| `ralph_runtime_state::blocked_marker_outcome` | Missing Ralph/absent blocked marker returns None; inspect only the supplied suffix; exact blocked notice/status/disable fields. | session/ralph_runtime_state::blocked_marker_outcome (unchanged body) |
| `ralph_runtime_state::handle_turn_completed` | No current gives idle; marker precedence Complete, Blocked, Waiting, then Interrupted, then Continue if not completed; same waiting/completed flag writes. | session/ralph_runtime_state::handle_turn_completed (unchanged body) |
| `ralph_runtime_state::RalphTurnOutcome::idle` | Default fields. | session/ralph_runtime_state::RalphTurnOutcome::idle (unchanged body) |
| `ralph_runtime_state::RalphTurnOutcome::complete` | Exact complete notice/status, disable true, no continuation. | session/ralph_runtime_state::RalphTurnOutcome::complete (unchanged body) |
| `ralph_runtime_state::RalphTurnOutcome::blocked` | Exact blocked notice/status, disable true, no continuation. | session/ralph_runtime_state::RalphTurnOutcome::blocked (unchanged body) |
| `ralph_runtime_state::RalphTurnOutcome::waiting` | Exact waiting notice/status, disable true, no continuation. | session/ralph_runtime_state::RalphTurnOutcome::waiting (unchanged body) |
| `ralph_runtime_state::RalphTurnOutcome::continuing` | No notice, continuing status, disable false, supplied continuation text. | session/ralph_runtime_state::RalphTurnOutcome::continuing (unchanged body) |
| `ralph_turn_queue::enqueue_turn_input` | Convert to Human enqueue with supplied history choice. | session/ralph_turn_queue::enqueue_turn_input (unchanged body) |
| `ralph_turn_queue::enqueue_prompt` | Ignore blank text; otherwise preserve source/history, keeps_ralph_pending=false, no deadline. | session/ralph_turn_queue::enqueue_prompt (unchanged body) |
| `ralph_turn_queue::queue_continuation` | Ignore blank text; one delayed Automatic continuation, no history, keeps_ralph_pending=true, Instant::now()+700ms. | session/ralph_turn_queue::queue_continuation (unchanged body) |
| `ralph_turn_queue::has_pending_continuation` | Delayed continuation or queued keeps_ralph_pending. | session/ralph_turn_queue::has_pending_continuation (unchanged body) |
| `ralph_turn_queue::pending_continuation_wait` | Nonempty ready queue returns None; otherwise saturating deadline wait, suppress zero. | session/ralph_turn_queue::pending_continuation_wait (unchanged body) |
| `ralph_turn_queue::pending_continuation_deadline` | Read optional stored deadline; test-only. | session/ralph_turn_queue::pending_continuation_deadline (unchanged body) |
| `ralph_turn_queue::promote_ready_continuation` | Only due deadline; take once, clear available_at, push at ready-queue back. | session/ralph_turn_queue::promote_ready_continuation (unchanged body) |
| `ralph_turn_queue::dequeue_turn_input` | Promote before pop_front; absent queue gives None. | session/ralph_turn_queue::dequeue_turn_input (unchanged body) |
| `ralph_turn_queue::queued_turn_inputs_is_empty` | Read ready queue; test-only. | session/ralph_turn_queue::queued_turn_inputs_is_empty (unchanged body) |
| `ralph_turn_queue::has_ready_queued_turn_input` | Ready queue wins; otherwise exact deadline <= now. | session/ralph_turn_queue::has_ready_queued_turn_input (unchanged body) |
| `ralph_turn_queue::next_input_origin` | Ready front first; otherwise only due delayed continuation. | session/ralph_turn_queue::next_input_origin (unchanged body) |
| `ralph::RalphState::new` | Store config and initialize primed/completed/waiting=false. | session/ralph::RalphState::new (unchanged body) |
| `ralph::load_ralph_config` | Resolve same relative/absolute path, read and trim; reject empty file/markers with same errors; retain exact default markers/continuation prompt. | session/ralph::load_ralph_config (unchanged body) |
| `ralph::detect_turn_markers` | Trim marker values; all empty returns immediately; scan exact suffix and only Assistant/Commentary; set all three booleans independently; early stop only when all true. | session/ralph::detect_turn_markers (unchanged body) |
| `ralph::text_contains_marker` | Exact trimmed line or marker followed by space+@@ suffix only. | session/ralph::text_contains_marker (unchanged body) |
| `ralph::resolve_prompt_path` | Absolute supplied path wins; otherwise cwd joins supplied/default path. | session/ralph::resolve_prompt_path (unchanged body) |
| `ralph::normalize_marker` | Trim, reject empty with same named error, return owned marker. | session/ralph::normalize_marker (unchanged body) |
| `approval_state::response_for_choice` | Reject unavailable one-time acceptance first; keep each kind-specific native JSON shape and capability guard, including no synthetic answers for field-bearing native questions. | session/approval_state::response_for_choice (unchanged body) |
| `approval_state::can_accept_once` | Missing decisions means true; supplied list must contain exact accept. | session/approval_state::can_accept_once (unchanged body) |
| `approval_state::empty_question_answer` | Missing payload, URL mode, required fields or nonempty schema properties means no empty answer; otherwise exact accept/content object. | session/approval_state::empty_question_answer (unchanged body) |
| `approval_state::decision_response` | Same exact decision strings; session/decline/cancel depend on offered flags. | session/approval_state::decision_response (unchanged body) |
| `approval_state::permissions_response` | Missing permissions defaults to empty object; session scope only when offered; decline sends empty permissions; other choices unavailable. | session/approval_state::permissions_response (unchanged body) |
| `approval_state::persists_after_turn_completed` | Only the same three Claude approval kinds persist. | session/approval_state::persists_after_turn_completed (unchanged body) |
| `approval_state::ApprovalState::new` | pending=None; no card allocation. | session/approval_state::ApprovalState::new (unchanged body) |
| `transcript_render::read_summary_path` | Trim and require arrow Read; empty path accepted; directory hints retain bracket contents; otherwise strip bracket detail; trim path. | session/read_summary::read_summary_path; read-only app reexports |
| `transcript_render::parse_read_summary` | Use final multiplication suffix; malformed count returns None; parsed count clamped to at least 1; absent suffix means 1. | session/read_summary::parse_read_summary; read-only app reexports |
| `transcript_render::format_read_summary_with_count` | Empty path formats arrow Read; add multiplication suffix only above 1. | session/read_summary::format_read_summary_with_count; read-only app reexports |

Per-file changes

- src/app/approval_state.rs: Replace the app-owned plain approval types with narrow session reexports; otherwise SessionState would depend on crate::app.

- src/app/backend_setup.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.

- src/app/backend_setup_grok.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.

- src/app/backend_setup_launch.rs: Update startup reads/writes to the new owners; no backend request, launch rule or default changed.

- src/app/claude_callbacks.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_context.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_edit.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_notices.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_notifications.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_prompts.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_question_input.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_reset.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/claude_session_history.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_callbacks.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_goal_receive.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_goal_text.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_goals.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_lifecycle.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_panel.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_panel_events.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_panel_pages.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_panel_receive.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_queue.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_queue_receive.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_reconnect.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_reconnect_install.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_reconnect_runtime.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_reconnect_streams.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_reply.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_settings.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/codex_task_status.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/context_usage.rs: Move only ContextUsage into the session and reexport it here; leave parsing and label formatting in place.

- src/app/input.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_dispatch.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_events.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_events_clipboard.rs: Hold the unchanged selection/last-answer copy shortcut with explicit imports.

- src/app/input_events_keys.rs: Read the new owners and move the unchanged copy shortcut to a companion to keep the file below 400 lines.

- src/app/input_events_model_settings.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_events_submit.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_mode.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/input_queue_rules.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/item_history.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/layout_state.rs: Change the content_x read to view.layout; no layout rule changed.

- src/app/mobile_mouse.rs: Change viewport field paths to view.viewport; retain every mouse rule and calculation.

- src/app/mod.rs: Register view_state; the temporary audit module/export was removed.

- src/app/mouse_events.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/native_questions.rs: Read pending approval through the immutable session accessor; retain modal/editor selection rules.

- src/app/notification_tools.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/notification_turns.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/notifications.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/overlay_render.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/overlay_render_model_settings.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/perf_bench.rs: Read the new owners and use explicit session writes while preserving benchmark dirty calls and measured work.

- src/app/perf_session.rs: Build/adopt through the session owner while retaining the same replay and benchmark behavior.

- src/app/ralph.rs: Reexport the test-facing plain Ralph types/constants after moving the actual automatic-loop facts/helpers to the session.

- src/app/ralph_runtime_state.rs: Reexport QueuedTurnInput after moving the runtime owner and its implementation into the session.

- src/app/ralph_turn_queue.rs: Delete the now-obsolete app-side queue implementation after moving its complete body under the session owner.

- src/app/render.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/render_input.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/render_sections.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/render_status.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/render_transcript.rs: Read view/session owners and delegate the unchanged active-reasoning-row lookup to the session.

- src/app/session_name.rs: Route thread/name assignments through session methods; retain Codex resets, safe-name parsing and native requests.

- src/app/state.rs: Construct SessionState and ViewState; retain six explicitly temporary mixed children and thin command/view adapters.

- src/app/state_dictation.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/state_input.rs: Move plain transcript/turn writes through session commands; keep composer, navigation, selection and rewind view rules.

- src/app/state_item_sources.rs: Pass concrete keys/native evidence and the original Claude-enabled guard to session source matching.

- src/app/state_items.rs: Build native keys and delegate item writes; preserve both reasoning dirty calls and the temporary display-part scope.

- src/app/state_ralph.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/state_settings.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/state_summaries.rs: Route label writes through session commands; retain summary worker, bindings, freshness checks and per-row dirty calls for M5.

- src/app/state_thread.rs: Split transcript/map resets from summary/history/backend/view steps while retaining their original visible order.

- src/app/state_transcript.rs: Keep layout/selection/cache helpers; apply session reports through the existing incremental dirty method; register the cache checks.

- src/app/state_transcript_checks.rs: Add 32 cache scenarios comparing incremental edits with fresh layout and complete line styles.

- src/app/task_badge.rs: Read the root thread id through the session accessor; retain every ownership/count rule.

- src/app/tool_output_command.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/tool_output_items.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/transcript_render.rs: Reexport the three unchanged plain read-label helpers from session/read_summary; Milestone 4 read grouping cannot import a renderer. This is the minimum helper extraction pulled forward from Milestone 5.

- src/app/turn_approval.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/turn_interrupt.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/turn_settings.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/turn_submit.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/turn_submit_history.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/app/view_state.rs: Define the terminal-only owner and preserve the existing widget/cache/default constructors.

- src/session/approval_state.rs: Move the unchanged plain approval request/choice/response model under session ownership.

- src/session/context_usage.rs: Define the same two plain token counts without importing app formatting helpers.

- src/session/item_sources.rs: Own Claude typed-source/cursor correlation, native evidence checks and superseded aliases.

- src/session/item_updates.rs: Own native command-label writes and reasoning normalization while preserving all tool facts.

- src/session/items.rs: Own native mapping/completion/placeholder and streamed/complete item writes, with original guards and return values.

- src/session/mod.rs: Declare the new terminal-free owners/helpers and register session tests in the session bridge.

- src/session/ralph.rs: Move unchanged prompt configuration/loading and marker detection without any terminal dependency.

- src/session/ralph_commands.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/session/ralph_runtime_state.rs: Move unchanged Ralph runtime facts and result/idle outcome rules.

- src/session/ralph_turn_queue.rs: Move unchanged human/automatic queue ordering and continuation deadlines.

- src/session/read_summary.rs: Move unchanged plain read-label/path/count parsing and formatting.

- src/session/reasoning.rs: Move unchanged streamed bold-heading text normalization.

- src/session/session_update_tests.rs: Enumerate earliest-entry, no-op, stream completion, full replacement, tail and duplicate-notice reports.

- src/session/state.rs: Own private transcript/thread/turn/label/approval/Ralph/context facts and expose immutable readers.

- src/session/state_commands.rs: Accept explicit scalar/card commands, preserve card identity and support existing test fixtures without mutable owner access.

- src/session/thread.rs: Own checked history adoption, fork/reset fact clearing and test-only suffix truncation reports.

- src/session/transcript_commands.rs: Keep exact low-level store writes behind explicit commands for still-mixed app bridges; preserve their original dirty-call timing.

- src/session/transcript_invariants.rs: Mechanically replace old AppState field paths with immutable session readers, explicit session writes or view fields; retain existing guards, defaults, request order and rendering rules.

- src/session/transcript_updates.rs: Own append/update/read-group/diff methods and define synchronous SessionUpdate reports.

- src/session/turn_boundary_tests.rs: Enumerate saved-suffix/first-entry lifetimes and empty/interrupted/Ralph marker combinations.

- src/session/turns.rs: Own empty-capable boundaries, turn lifecycle, Ralph result handling and active-reasoning facts.

- src/tests/approval_identity_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_backend_tests/local_history_recovery.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_backend_tests/normalized_app.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_backend_tests/review_receiver_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_backend_tests/translation_and_submit.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_context_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_edit_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_native_state_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_question_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_sdk_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_session_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_thinking_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/claude_visibility_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_goal_lifecycle_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_goal_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_history_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_panel_read_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_panel_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_question_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_queue_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_reconnect_identity_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_reconnect_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_settings_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_steer_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/codex_task_status_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_fork_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_history_identity_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_identity_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_lookup_differential_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_lookup_extra_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_review_cursor_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/entry_review_history_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/grok_backend_tests/history_and_submit.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/input_status_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/input_tests/approvals_and_rewind.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/input_tests/dictation.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/input_tests/mobile_mouse_and_perf.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/native_activity_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/notification_tests/context_and_approvals.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/notification_tests/diff_notifications.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/notification_tests/history_and_tools.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/phone_layout_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/rtk_transcript_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/runtime_tests/ralph_runtime.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/session_name_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/summary_render_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/summary_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/three_c_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/tool_command_view_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/tool_output_item_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/tool_output_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/tool_tests/command_summaries.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/tool_tests/shell_rendering.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/transcript_spacing_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/turn_lifecycle_tests.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/ui_render_tests/drawing_and_wrapping.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/ui_render_tests/markdown_and_diff.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/ui_render_tests/source_highlight.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

- src/tests/ui_render_tests/transcript_and_input.rs: Mechanically adapt existing fixture reads/writes to the new owners and explicit commands; preserve its original assertions and tested behavior.

Remaining limits

I found no unexplained mutation difference, failing required command, new compiler warning or requested speed-gate failure. The generated corpus does not prove parity for every possible backend frame/thread schedule. The known baseline panic, deadline flake, broken RSS metric and the nine pre-existing overlong caller functions are recorded above. I did not fix those unrelated behaviors or implement any later milestone. I left the source and the requested report for review without committing or installing Carlos.

