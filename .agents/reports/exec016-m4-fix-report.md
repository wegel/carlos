Carlos milestone 4 fix report, 2026-11-12 UTC

I fixed both review findings in the uncommitted milestone 4 change above HEAD 0f5f2b4. Local notices now go to ViewState. ViewState also holds the second-consent key and the extra approval-card lines. SessionState keeps backend/turn/Ralph status and the backend's original pending request. Both reviewer probes are permanent tests, with full session snapshots added before and after the local action.

I also fixed the id-less JSON-map panic with a checked field lookup. The independent patch is /tmp/carlos-idless-panic-fix.patch. No commit, push, stash, Git reset, install, dependency change, or change under .agents/ or docs/ was made. The main checkout kept its existing Rust 1.100.0 override.

One requested gate cannot pass while preserving HEAD: 86 of the 1,244 formerly panicking full streams already behave differently from a59221c in committed code. HEAD plus only the standalone panic patch matches the corrected working tree on all 1,244 streams. The pre-plan comparison and the exact remaining differences appear below. I did not revert those committed behaviors.

Status and hint storage

HEAD never renders its general AppState.status string. `git grep '(app|self)\.status' HEAD -- src` finds its writers and tests, but no renderer read. The real status bar renders model, quota, activity, and input-mode labels. I preserved that screen instead of adding a new status label. AppState::status_text is a test-only view helper that reads the same last-written string as HEAD; both mutation oracles also capture the real status band's text.

ViewState.status_hint holds a local string and the session's status revision when the local action wrote it. A backend or turn status write increments SessionState.status_revision, including a repeated write of identical text. The view therefore ignores an older hint after any later session status write. Ralph's internal status writes and set_pending_approval use the same setter. A local setter reads the revision and writes only the view. An empty local hint remains a real write. No transcript scan, transcript clone, terminal import in src/session/, Deref adapter, RefCell, or new dependency was added.

HEAD does not clear this general status string on the next ordinary key. The tests preserve that fact. These sequences are in the HEAD golden fixture:

- Backend “turn completed”, then real Ctrl+L: the view reads “selection cleared”; session status and every other session field stay unchanged.
- A normal x key after Ctrl+L: the text stays “selection cleared”, as on HEAD.
- Another backend write of “turn completed”, even with identical text: the local hint stops winning.
- A dictation endpoint choice after that backend write: the device notice wins without changing session facts.
- Backend “turn failed” while a local hint exists: backend text wins.
- An empty local write after backend text: the empty string wins.
- An internal Ralph disable after a local write: “ralph off” wins.
- A pending approval, backend turn events during a hint, more local keys, and later approval answers: the augmented mutation corpus compares every event checkpoint.

The caller table below covers every current call to set_status or set_hint. It includes the app forwarding method and the session's internal writers. For a helper with both causes, separate rows identify the branch. `session_name::report_rename_error` uses a local branch for unsupported-backend/input checks and a session branch for a failed backend rename. `turn_approval::reply_error` does the same for an invalid field versus a failed backend reply. `codex_callbacks::reply_hint` handles a local invalid choice/field; reply_error additionally records a failed backend operation. Save failures for local preferences/defaults and all dictation notices are view hints.

The audit found 103 setter calls.

| Caller | Owner | Write |
| --- | --- | --- |
| `src/app/backend_setup.rs:212` / `configure_app_common` | Session, backend/turn/Ralph fact | `app.set_status("ready");` |
| `src/app/backend_setup_launch.rs:297` / `configure_claude_app` | Session, backend/turn/Ralph fact | `app.set_status("Claude starting");` |
| `src/app/claude_models.rs:48` / `apply_capabilities` | Session, backend/turn/Ralph fact | `app.set_status("Claude ready");` |
| `src/app/claude_notifications.rs:203` / `handle_event` | Session, backend/turn/Ralph fact | `app.set_status(format!("Claude closed: {reason}"));` |
| `src/app/claude_notifications.rs:239` / `fail_accepted_prompts` | Session, backend/turn/Ralph fact | `app.set_status("turn failed");` |
| `src/app/claude_notifications.rs:248` / `show_error` | Session, backend/turn/Ralph fact | `app.set_status(message);` |
| `src/app/claude_question_input.rs:80` / `interrupt_native` | Session, backend/turn/Ralph fact | `Ok(_) => app.set_status("native interrupt requested"),` |
| `src/app/claude_question_input.rs:81` / `interrupt_native` | Session, backend/turn/Ralph fact | `Err(error) => app.set_status(format!("native interrupt failed: {error}")),` |
| `src/app/claude_question_input.rs:237` / `copy_browser_link` | View, local hint | `app.set_hint("browser link copied; approve only if you consent to this server's request");` |
| `src/app/claude_reset.rs:31` / `receive` | Session, backend/turn/Ralph fact | `app.set_status("Claude cleared the conversation; waiting for its native session identity");` |
| `src/app/claude_rewind.rs:12` / `toggle_unknown` | View, local hint | `app.set_hint("Edit human prompt; F4 selects older prompts with unknown source");` |
| `src/app/claude_rewind.rs:16` / `toggle_unknown` | View, local hint | `app.set_hint("No older prompts with unknown source");` |
| `src/app/claude_rewind.rs:40` / `select` | View, local hint | `app.set_hint( "Source unknown: Enter forks before this prompt; F4 human prompts; Esc restores draft",` |
| `src/app/claude_session_history.rs:58` / `switch_session` | Session, backend/turn/Ralph fact | `app.set_status("Claude starts the edited branch");` |
| `src/app/claude_session_history.rs:72` / `receive` | Session, backend/turn/Ralph fact | `app.set_status("Reading Claude history");` |
| `src/app/codex_callbacks.rs:187` / `activate` | Session, backend/turn/Ralph fact | `app.set_status("Codex has a pending request; F4 answers it");` |
| `src/app/codex_callbacks.rs:355` / `reply_hint` | View, local hint | `app.set_hint(safe_text(error));` |
| `src/app/codex_callbacks.rs:361` / `reply_error` | Session, backend/turn/Ralph fact | `app.set_status(safe_text(error));` |
| `src/app/codex_lifecycle.rs:45` / `close` | Session, backend/turn/Ralph fact | `app.set_status(format!("{label}: {message}. F5 reconnects"));` |
| `src/app/codex_lifecycle.rs:61` / `reject_closed_prompt` | View, local hint | `app.set_hint("Codex is closed; F5 reconnects; draft kept");` |
| `src/app/codex_reconnect.rs:27` / `start` | Session, backend/turn/Ralph fact | `app.set_status( "Reconnecting Codex; draft kept. Messages wait for the complete native snapshot",` |
| `src/app/codex_reconnect.rs:31` / `start` | Session, backend/turn/Ralph fact | `Err(error) => app.set_status(format!("Could not reconnect Codex: {error}")),` |
| `src/app/codex_reconnect.rs:177` / `finish_ui` | Session, backend/turn/Ralph fact | `app.set_status("Codex reconnected; native work restored. Unconfirmed input was not resent");` |
| `src/app/codex_reconnect_runtime.rs:30` / `tick` | Session, backend/turn/Ralph fact | `app.set_status(format!("Could not refresh Codex thread settings: {error}"));` |
| `src/app/codex_reconnect_runtime.rs:52` / `reply` | Session, backend/turn/Ralph fact | `app.set_status( "Could not refresh Codex thread settings; native defaults still govern prompts",` |
| `src/app/codex_reconnect_streams.rs:197` / `tick` | Session, backend/turn/Ralph fact | `app.set_status(format!( "Could not refresh partial Codex output: {error}; waiting for its complete event"` |
| `src/app/codex_reply.rs:80` / `send` | Session, backend/turn/Ralph fact | `app.set_status("Answer sent");` |
| `src/app/input_dispatch.rs:104` / `dispatch_action` | Session, backend/turn/Ralph fact | `app.set_status(format!("Claude session failed: {error}"));` |
| `src/app/input_dispatch.rs:118` / `dispatch_action` | Session, backend/turn/Ralph fact | `app.set_status(format!("Could not apply Claude settings: {error}"));` |
| `src/app/input_dispatch.rs:122` / `dispatch_action` | Session, backend/turn/Ralph fact | `app.set_status(format!("Could not read context: {error}"));` |
| `src/app/input_dispatch.rs:131` / `dispatch_action` | Session, backend/turn/Ralph fact | `app.set_status(format!("server request reply failed: {err}"));` |
| `src/app/input_dispatch.rs:136` / `dispatch_action` | Session, backend/turn/Ralph fact | `app.set_status(format!("Could not read context: {error}"));` |
| `src/app/input_events_clipboard.rs:16` / `handle_ctrl_y_copy` | View, local hint | `app.set_hint(format!( "copied last assistant message ({})",` |
| `src/app/input_events_keys.rs:298` / `handle_ctrl_r_ralph` | View, local hint | `app.set_hint(format!("ralph: {e}"));` |
| `src/app/input_events_keys.rs:306` / `handle_ctrl_l_clear_selection` | View, local hint | `app.set_hint("selection cleared");` |
| `src/app/input_events_model_settings.rs:124` / `save_settings` | Session, backend/turn/Ralph fact | `app.set_status("Claude settings applied");` |
| `src/app/input_events_model_settings.rs:128` / `save_settings` | Session, backend/turn/Ralph fact | `Err(error) => app.set_status(format!("Could not apply live settings: {error}")),` |
| `src/app/input_events_model_settings.rs:133` / `save_settings` | View, local hint | `app.set_hint(format!("Could not save AI tool labels: {error}"));` |
| `src/app/input_events_model_settings.rs:146` / `save_settings` | View, local hint | `app.set_hint(format!("saved for next turn; default save failed: {err}"));` |
| `src/app/input_events_submit.rs:24` / `handle_enter_submit` | View, local hint | `app.set_hint(if app.codex.connection.rebuilding {` |
| `src/app/notification_turns.rs:23` / `handle_turn_started` | Session, backend/turn/Ralph fact | `app.set_status("turn started");` |
| `src/app/notification_turns.rs:70` / `handle_turn_completed` | Session, backend/turn/Ralph fact | `app.set_status("turn interrupted");` |
| `src/app/notification_turns.rs:74` / `handle_turn_completed` | Session, backend/turn/Ralph fact | `app.set_status("turn failed");` |
| `src/app/notification_turns.rs:76` / `handle_turn_completed` | Session, backend/turn/Ralph fact | `app.set_status("turn completed");` |
| `src/app/notification_turns.rs:82` / `handle_turn_completed` | Session, backend/turn/Ralph fact | `app.set_status(format!("ralph: {e}"));` |
| `src/app/notifications.rs:321` / `dispatch_notification` | Session, backend/turn/Ralph fact | `app.set_status(msg);` |
| `src/app/session_name.rs:125` / `save_name` | Session, backend/turn/Ralph fact | `app.set_status("session renamed");` |
| `src/app/session_name.rs:134` / `report_rename_error` | Session, backend/turn/Ralph fact | `app.set_status(message.clone());` |
| `src/app/session_name.rs:136` / `report_rename_error` | View, local hint | `app.set_hint(message.clone());` |
| `src/app/state.rs:92` / `set_status` | Session, backend/turn/Ralph fact | `self.session.set_status(s)` |
| `src/app/state_dictation.rs:66` / `toggle_dictation_endpoint_mode` | View, local hint | `self.set_hint(err);` |
| `src/app/state_dictation.rs:74` / `toggle_dictation_endpoint_mode` | View, local hint | `self.set_hint(match mode {` |
| `src/app/state_dictation.rs:116` / `start_dictation_recording` | View, local hint | `self.set_hint("dictation unavailable while turn is active");` |
| `src/app/state_dictation.rs:121` / `start_dictation_recording` | View, local hint | `Err(err) => self.set_hint(err),` |
| `src/app/state_dictation.rs:144` / `cancel_dictation` | View, local hint | `self.set_hint("dictation cancelled");` |
| `src/app/state_dictation.rs:161` / `switch_dictation_profile_next` | View, local hint | `self.set_hint(format!("dictation profile: {name}"));` |
| `src/app/state_dictation.rs:163` / `switch_dictation_profile_next` | View, local hint | `Err(err) => self.set_hint(format!("dictation profile unavailable: {err}")),` |
| `src/app/state_dictation.rs:169` / `switch_dictation_profile_next` | View, local hint | `self.set_hint("dictation unavailable: rebuild with --features dictation");` |
| `src/app/state_dictation.rs:181` / `commit_dictation_final` | View, local hint | `self.set_hint("dictation inserted");` |
| `src/app/state_dictation.rs:210` / `start_dictation_capture` | View, local hint | `self.set_hint("dictation recording");` |
| `src/app/state_dictation.rs:216` / `start_dictation_capture` | View, local hint | `self.set_hint("dictation unavailable: event channel is not ready");` |
| `src/app/state_dictation.rs:226` / `start_dictation_capture` | View, local hint | `self.set_hint("dictation recording");` |
| `src/app/state_dictation.rs:230` / `start_dictation_capture` | View, local hint | `self.set_hint(format!("dictation capture unavailable: {err}"));` |
| `src/app/state_dictation.rs:237` / `start_dictation_capture` | View, local hint | `self.set_hint("dictation recording");` |
| `src/app/state_dictation.rs:243` / `finish_dictation_capture` | View, local hint | `self.set_hint("dictation transcribing");` |
| `src/app/state_dictation.rs:250` / `finish_dictation_capture` | View, local hint | `self.set_hint(format!("dictation capture failed: {err}"));` |
| `src/app/state_dictation.rs:257` / `finish_dictation_capture` | View, local hint | `self.set_hint("dictation transcribing");` |
| `src/app/state_dictation.rs:278` / `handle_dictation_capture_error` | View, local hint | `self.set_hint(format!("dictation capture failed: {err}"));` |
| `src/app/state_dictation.rs:286` / `request_dictation_transcription` | View, local hint | `self.set_hint(format!("dictation transcribing ({sample_count} samples)"));` |
| `src/app/state_dictation.rs:291` / `request_dictation_transcription` | View, local hint | `self.set_hint(format!("dictation transcribing ({sample_count} samples)"));` |
| `src/app/state_dictation.rs:303` / `request_dictation_transcription` | View, local hint | `self.set_hint("dictation transcribing");` |
| `src/app/state_dictation.rs:307` / `request_dictation_transcription` | View, local hint | `self.set_hint(format!("dictation worker unavailable: {err}"));` |
| `src/app/state_dictation.rs:323` / `handle_dictation_transcription_final` | View, local hint | `self.set_hint("dictation inserted");` |
| `src/app/state_dictation.rs:335` / `handle_dictation_transcription_error` | View, local hint | `self.set_hint(format!("dictation transcription failed: {message}"));` |
| `src/app/state_dictation.rs:346` / `handle_dictation_transcription_cancelled` | View, local hint | `self.set_hint("dictation cancelled");` |
| `src/app/state_input.rs:71` / `enter_rewind_mode` | View, local hint | `self.set_hint("Edit human prompt; F4 selects older prompts with unknown source");` |
| `src/app/state_ralph.rs:19` / `request_ralph_toggle` | Session, backend/turn/Ralph fact | `self.set_status(status);` |
| `src/app/state_ralph.rs:33` / `apply_pending_ralph_toggle` | Session, backend/turn/Ralph fact | `self.set_status(status);` |
| `src/app/state_settings.rs:133` / `apply_model_settings` | View, local hint | `self.set_hint(format!("{label} pending next turn; saved as default"));` |
| `src/app/state_settings.rs:135` / `apply_model_settings` | View, local hint | `self.set_hint(format!("{label} set for next turn; saved as default"));` |
| `src/app/state_summaries.rs:209` / `handle_summary_event` | Session, backend/turn/Ralph fact | `self.set_status(format!("AI tool label unavailable: {reason}"))` |
| `src/app/turn_approval.rs:45` / `respond_to_pending_approval` | Session, backend/turn/Ralph fact | `app.set_status(status);` |
| `src/app/turn_approval.rs:81` / `build_answer` | View, local hint | `app.set_hint("approval choice not available");` |
| `src/app/turn_approval.rs:107` / `confirm_default_no` | View, local hint | `app.set_hint(hint);` |
| `src/app/turn_approval.rs:131` / `reply_error` | Session, backend/turn/Ralph fact | `app.set_status(error);` |
| `src/app/turn_approval.rs:133` / `reply_error` | View, local hint | `app.set_hint(error);` |
| `src/app/turn_interrupt.rs:33` / `interrupt_active_turn` | Session, backend/turn/Ralph fact | `app.set_status("interrupt requested");` |
| `src/app/turn_interrupt.rs:39` / `interrupt_active_turn` | Session, backend/turn/Ralph fact | `app.set_status(format!("{e}"));` |
| `src/app/turn_submit.rs:59` / `submit_rewind_turn_text` | Session, backend/turn/Ralph fact | `app.set_status(format!("fork failed: {err}"));` |
| `src/app/turn_submit.rs:78` / `submit_rewind_turn_text` | Session, backend/turn/Ralph fact | `app.set_status(format!("fork failed: {err}"));` |
| `src/app/turn_submit.rs:169` / `submit_turn` | View, local hint | `app.set_hint("Claude is closed; restart Carlos to send this draft");` |
| `src/app/turn_submit.rs:178` / `submit_turn` | Session, backend/turn/Ralph fact | `app.set_status("queued while Claude starts");` |
| `src/app/turn_submit.rs:233` / `steer_existing_turn` | Session, backend/turn/Ralph fact | `app.set_status("sent steer");` |
| `src/app/turn_submit.rs:235` / `steer_existing_turn` | Session, backend/turn/Ralph fact | `Err(e) => app.set_status(format!("{e}")),` |
| `src/app/turn_submit.rs:250` / `start_new_turn` | View, local hint | `app.set_hint("Claude is starting; answer any startup question first");` |
| `src/app/turn_submit.rs:291` / `start_new_turn` | Session, backend/turn/Ralph fact | `app.set_status(format!("send failed: {err}"));` |
| `src/app/turn_submit.rs:311` / `queue_behind_pending` | Session, backend/turn/Ralph fact | `app.set_status(match client.kind() {` |
| `src/app/turn_submit.rs:347` / `mark_turn_sent` | Session, backend/turn/Ralph fact | `app.set_status("sent turn");` |
| `src/session/state_commands.rs:44` / `set_pending_approval` | Session, backend/turn/Ralph fact | `self.set_status(format!("approval requested: {}", approval.title));` |
| `src/session/turns.rs:77` / `enable_ralph_mode` | Session, backend/turn/Ralph fact | `self.set_status(status);` |
| `src/session/turns.rs:83` / `disable_ralph_mode` | Session, backend/turn/Ralph fact | `self.set_status(status);` |
| `src/session/turns.rs:221` / `maybe_disable_ralph_on_blocked_marker` | Session, backend/turn/Ralph fact | `self.set_status(status);` |
| `src/session/turns.rs:254` / `handle_ralph_turn_completed` | Session, backend/turn/Ralph fact | `self.set_status(status);` |

Direct assignments and constructors of other fields named status were checked as well. They are listed separately so a picker message cannot be mistaken for the session status.

| Writer | Owner | Write |
| --- | --- | --- |
| `src/app/claude_tasks.rs:214` / `merge_edge` | Session/backend or Ralph outcome | `record.status = TaskStatus::parse(patch["status"].as_str()).unwrap_or_else(&#124;&#124; {` |
| `src/app/claude_tasks.rs:319` / `insert` | Session/backend or Ralph outcome | `status: TaskStatus::Unknown,` |
| `src/app/codex_goal_state.rs:68` / `new` | View, picker or unsent editor | `status: goal.is_null().then(&#124;&#124; "paused".into()),` |
| `src/app/codex_goal_view.rs:98` / `editor_key` | View, picker or unsent editor | `draft.status = Some(` |
| `src/app/codex_rollout.rs:122` / `end_turn` | Session/backend or Ralph outcome | `self.turns[turn_index].status = status.to_string();` |
| `src/app/codex_rollout.rs:167` / `ensure_turn` | Session/backend or Ralph outcome | `status: "inProgress".to_string(),` |
| `src/app/terminal_ui.rs:179` / `pick_thread` | View, picker or unsent editor | `status: None,` |
| `src/app/terminal_ui_picker_events.rs:45` / `handle_delete_confirm_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:65` / `execute_delete` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:72` / `execute_delete` | View, picker or unsent editor | `st.status = Some(format!("Archived {}", thread.id));` |
| `src/app/terminal_ui_picker_events.rs:79` / `execute_delete` | View, picker or unsent editor | `st.status = Some(format!("archive failed: {err}"));` |
| `src/app/terminal_ui_picker_events.rs:96` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:101` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:106` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:110` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:114` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:118` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:122` / `handle_normal_key` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:141` / `handle_picker_mouse` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:145` / `handle_picker_mouse` | View, picker or unsent editor | `st.status = None;` |
| `src/app/terminal_ui_picker_events.rs:163` / `handle_mouse_click` | View, picker or unsent editor | `st.status = None;` |
| `src/session/ralph_runtime_state.rs:70` / `complete` | Session/backend or Ralph outcome | `status: Some("ralph complete".to_string()),` |
| `src/session/ralph_runtime_state.rs:80` / `blocked` | Session/backend or Ralph outcome | `status: Some("ralph blocked".to_string()),` |
| `src/session/ralph_runtime_state.rs:90` / `waiting` | Session/backend or Ralph outcome | `status: Some("ralph waiting".to_string()),` |
| `src/session/ralph_runtime_state.rs:100` / `continuing` | Session/backend or Ralph outcome | `status: Some("ralph continuing".to_string()),` |
| `src/session/ralph_runtime_state.rs:265` / `blocked_marker_outcome` | Session/backend or Ralph outcome | `status: Some("ralph blocked".to_string()),` |
| `src/session/state.rs:44` / `new` | Session/backend or Ralph outcome | `status: String::new(),` |
| `src/session/state_commands.rs:10` / `set_status` | Session/backend or Ralph outcome | `self.status = text.into();` |

Other status writers checked

- SessionState::new initializes status and its revision. SessionState::set_status is the only assignment to the stored general status string. set_pending_approval calls it instead of assigning the field directly.
- ViewState::new initializes status_hint. AppState::set_hint is its only write after construction. The view helper chooses it by revision, without clearing any backend fact.
- RalphTurnOutcome constructors emit backend-independent automation facts for complete, blocked, waiting, continuing, and failure. The session applies those outcomes through set_status. They do not store terminal hints.
- terminal_ui::PickerState.status and every st.status write in terminal_ui_picker_events remain picker-owned. Navigation/selection clears the picker's text; archive replies supply picker notices. No picker field moved into SessionState.
- codex_goal_state::GoalDraft.status and codex_goal_view's cycle write are the unsent goal editor's draft, still outside SessionState for milestone 8. They are not the general status message.
- codex_rollout::Turn.status/end_turn, Claude task records' status/update, TurnStatus maps, and history/native-activity status parsing describe backend work. They remain backend facts. Locals named status in perf_session, native_activity, or task/goal rendering are not general-status writes.

Approval cards and consent sequences

ViewState.confirm_allow replaces ClaudeUiState.confirm_allow without changing its existing reset guards. ApprovalHints stores inserted lines by Carlos card EntryId, which also distinguishes a new request from a retired request with reused native text. The two card renderers read local lines before the request's unchanged detail_lines. They do not clone the transcript or insert display text into the stored request. Cancel/retire/clear removes that card's local lines. A focus move retains the old card's lines but resets the confirmation key exactly where HEAD did. This preserves HEAD's repeated prepended line if the user returns to a card and starts consent again.

The permanent golden test covers all ten approval kinds with Codex, Claude, and Grok client labels. It also uses real Claude permission/AskUserQuestion drafts and all seven Codex callback methods (command, file, permissions, both legacy approvals, structured question, MCP form). It checks the stored effective status, full rendered-card cell/style hash, exact backend calls, focused native request, and confirmation key. The fixture contains 276 cases: three status sequences and 273 approval sequences. For each applicable kind/backend it enumerates:

- First Accept: send immediately if HEAD did; otherwise retain every backend request field and show the same second-consent line. An unavailable choice keeps the same error and sends nothing.
- Second Accept: send the same native answer once, retire the same card, and show the same accepted status.
- Decline after first Accept: send the same decline once and clear only that card.
- Another key after first Accept: preserve or edit the same local draft; require exactly the consent flow HEAD used.
- Focus away and back through real native callbacks: keep the first card's hint text, reset the same key flag, and reproduce the repeated line on another first Accept.
- A second approval arriving: retain the active card and queue the same second request.
- Backend cancellation or resolution during confirmation: remove that request; another Accept must not answer a retired request.

The two original reviewer probes remain under src/tests/view_ownership_tests.rs. Additional tests compare Debug snapshots of every SessionState field, including transcript ids/maps/allocator, turn boundary, Ralph queues, pending approval, context, status, and revision. They cover first consent for all ten kinds and all three client labels, dictation/local editor hints, and real native permission/answer/form drafts. Test-only Debug derives enable these checks; production code gains no clone.

Two terminal workflows were added to the existing test files. They assert the unchanged completed-turn screen and no extra backend send after Ctrl+L/typing, and the visible “Press y again” card plus exact native replies for another key followed by decline or second Accept. Existing phone, focus, cancellation, and concurrent-callback terminal tests also pass.

Search for the same ownership defect

I used semeja search by intent, followed related state commands, then used exhaustive literal searches for status, confirmation, hints, mutable session fields, session command callers, and terminal references. I inspected every field in SessionState and every method in the new session modules:

- state/state_commands: thread identity/name, context, active turn, pending request, status, status_revision. The old prepend_approval_hint was the second defect and is removed. I moved confirm_allow into the view rather than leaving it in the temporary Claude wrapper.
- turns/ralph_commands/ralph_runtime_state/ralph_turn_queue: accepted/queued prompts, human/automatic origins, markers, prompt paths, delay deadlines, and toggle/work state control the session even without a terminal. A key can request real automation work; a local error about a refused toggle now uses set_hint.
- transcript/transcript_aliases/history_identity/entry_id/entry_key: rows, source keys, id allocator, aliases, part counts, active/completed facts, history replacement, and rewind retirement. These are transcript facts; no terminal position, focus, width, or widget entered the store.
- items/item_sources/item_updates/reasoning: backend item scope, source proof, completion guards, reasoning text, parsed command labels, and updates to stored output. Their strings come from backend records or accepted transcript content. Their indices resolve stored rows, not terminal coordinates.
- transcript_commands/transcript_updates/thread/read_summary: explicit store writes, read-label grouping, turn-diff changes, history/fork reset, and earliest changed-entry reports. SessionUpdate carries entry ids/reset flags, not terminal rows. active_reasoning_step is a read-only query over backend work.
- approval_state/models/context_usage: plain backend request/answer types, transcript drafts/details, and token counts. Existing initial guidance derived from defaultToNo comes from the backend request; the extra line triggered by the first local Accept now lives only in the view.
- AppState's six explicitly temporary mixed children (summaries, input_history, runtime, rate_limits, claude, codex) retain the milestone 5-9 boundaries. I did not move their remaining widget/editor fields into SessionState or implement later milestones.

I found no third view-only field in the newly created session owner. The status audit did find more local callers than the two reviewer examples: every dictation notice, browser-copy text, rewind hints, closed/startup submission guards, invalid approval choices/fields, local setting/default-save messages, and invalid rename input now use the view setter. Backend send/control/reply failures and accepted turn/Ralph/queue facts still use session status.

The source under src/session/ contains no crate::app, ratatui, crossterm, or RefCell reference. Files stay at or below 400 lines. No new function exceeds 60 lines. The original long caller functions retain their prior lengths; the previous reviewer inventory and target/m4-fix/function-sizes.json record those existing cases.

Independent panic fix

complete_tool_item now uses item.get("id").and_then(Value::as_str) before retiring keyed diff parts. It then runs the existing no-id fallback. A missing id therefore does not panic and does not drop readable unkeyed content.

The independently committable HEAD patch touches exactly:

- src/app/notification_tools.rs: the single id lookup in complete_tool_item, plus the final cfg(test) module hook. The neighboring app.messages-to-session adapter belongs to milestone 4, not the panic fix. Patch hunk headers: @@ -42,7 +42,7 @@ for the checked lookup and @@ -132,3 +132,7 @@ for the test hook.
- src/tests/idless_item_tests.rs: the entire new 113-line optional-id row test, patch hunk @@ -0,0 +1,113 @@. The standalone patch borrows &app.messages; the working tree's one owner-access line borrows app.session.messages(), which belongs to milestone 4.
- src/tests/fixtures/idless_pre_plan.json: the entire pre-plan row fixture, patch hunk @@ -0,0 +1 @@ (2,880 JSON records on one line).

The ordinary git diff groups the checked id lookup with its adjacent milestone 4 adapter. The supplied /tmp/carlos-idless-panic-fix.patch separates them and can be applied to HEAD before committing milestone 4. I applied it to the detached HEAD worktree, checked its reverse application, compiled it, and passed its full permanent row test. No staging or commit was performed in the user's checkout.

The requested /tmp/carlos-pre-plan worktree was created detached at a59221c and used Rust 1.100.0 through its own directory override. The reference test calls the old real app handler, captures the actual row role/kind/text/path/tool-details, and checks that no native key is created. It does not substitute the new handler on the reference side. The current permanent test compares against those captured rows.

The row fixture has 2,880 cases: 20 item labels, three backend selections, item/started and item/completed, eight content variants, and missing/null/numeric ids. It includes every command/tool-call/tool-output alias accepted by the handler, reasoning, agent/user messages, compaction, sleep/subagent/collaboration activity, plus fileChange and message as explicit ignored-label controls. Variants cover bare items, output, readable Read names/paths, command output, one diff, multiple file diffs, reasoning/text, and content arrays.

All cases match a59221c. Bare tool calls are ignored. Read names, command output, tool output, and supported diff-bearing tool items append unkeyed rows. Started items without ids remain ignored. One detail of the actual reference: a literal type:fileChange item is ignored by this handler even when it carries diffs, at both a59221c and HEAD. The tool-result/command labels carrying file diffs append their unkeyed diff rows. I preserved that observed behavior rather than add support for another item label.

Mutation parity and the pre-plan limit

The original target/m4 harness and exact corpus were reused. The augmented oracle uses the same dispatch, Claude normalizer/client, Grok translator, mock replies, and dirty hook. Its canonical output adds the real visible status band, effective general-status text, and approval-card text plus a hash of every card cell and style. Current request details are projected through the view when comparing HEAD's visible card; separate permanent tests assert that raw session request facts remain untouched. This explicitly accounts for the intended ownership split rather than treating stored local text as a backend fact.

The original 24,000 streams retain their inputs. Another 300 deterministic local key/callback/confirmation schedules extend the corpus. Current and HEAD are compared after every event and at stream end, including cached transcript lines/styles, complete ordered dirty traces, backend calls, normalized entry relationships, status, approval cards, and checkpoint hashes.

Augmented result: 24,300 streams; 23,056/23,056 nonpanicking HEAD streams match exactly; 1,244 original HEAD panics; 0 current panics; no unexplained HEAD-parity difference. The original nonpanic subset is 22,756 and the added local subset is 300. Evidence: target/m4-fix/comparison.json, head.jsonl/current.jsonl, head-full.rs, current-full.rs, install.py, run.py, and shard logs.

I also replayed every one of the 1,244 former panic streams, in full, against a59221c. Because the old commit has no Carlos EntryId store, this comparison removes only identity-specific fields and retains all row values, turn/Ralph/approval/context/status facts, backend calls, dirty traces, rendered lines/styles, card cells, status bands, and per-event checkpoints. Every old panic input is Codex; the reference refuses a Claude native-frame operation rather than mock its translation.

Pre-plan full-stream result: 1,244 streams, no panic on either side; 1,158 exact results; 1,206 identical final row sets; 86 different full results, including 38 different final row sets. Of the 86, 45 differ only in dirty traces. The other 41 have a first different intermediate checkpoint at item/completed (38), agentMessage/delta (2), or item/started (1). These are not silently waived as successful pre-plan matches.

To establish the cause, I built HEAD with only the standalone panic patch and the same temporary oracle. It matches the corrected milestone 4 tree on 1,244/1,244 formerly panicking streams, including every semantic checkpoint, dirty call, rendered line, status band, and approval card. This isolates all 86 old-commit differences to already committed code. Example stream 114 first differs on a keyed reasoning completion at event 5, before the crashing id-less tool item at event 17. Stream 798 also differs on keyed assistant delta scope; stream 1197 differs on a keyed reasoning completion before its id-less Read item. Reverting those behaviors would violate the requirement to preserve HEAD except for the panic.

The remaining unmet gate is exact a59221c parity for those 86 full streams. The single-item fallback test does match a59221c in every case, and the full corrected streams match HEAD plus only the panic patch in every case. Evidence: preplan-comparison.json, preplan-panic.jsonl, current-panic.jsonl, head-fixed-comparison.json, head-fixed-panic.jsonl, inherited-differences.json, preplan-first-difference.json, and their compile/run logs under target/m4-fix/.

Checks

- build: exit 0, 2.818 s. Full log: target/m4-fix/build-final.log.

- build-release: exit 0, 47.181 s. Full log: target/m4-fix/build-release-final.log.

- fmt: exit 0, 0.375 s. Full log: target/m4-fix/fmt-final.log.

- style: exit 0, 1.840 s. Full log: target/m4-fix/style-final.log.

- test-compile: exit 0, 6.669 s. Full log: target/m4-fix/test-compile-final.log.

- Final cargo test run 1: exit 0, 4.327 s wall; 834 passed, 2 ignored.

- Final cargo test run 2: exit 0, 3.920 s wall; 834 passed, 2 ignored.

- Final cargo test run 3: exit 0, 3.864 s wall; 834 passed, 2 ignored.

- bash scripts/test.sh: passed all 173 terminal workflows, 80.305 s for the terminal portion, plus Rust tests, both controller test runs, recorder and package smoke. Artifacts: target/ui-artifacts/run-iNfaay/. Log: target/m4-fix/terminal-final.log. The stronger final Rust-only fixtures were recaptured and checked in the final three cargo test runs afterward; executable code did not change.
- No new compiler warning. The unchanged deprecated AtomicUsize::fetch_update warning remains in src/protocol_writer.rs:72. Pre-plan builds have their three original warnings. Temporary oracle exports can add an unused finish warning only while injected; no oracle hook remains in src/.
- One earlier final-suite attempt failed the unchanged protocol test a_native_event_flood_fails_pending_work_and_stops_the_process_without_a_consumer: its try_recv loop observed zero reserved diagnostics instead of one. The following suites passed. Twenty isolated runs of that exact test on HEAD plus only the panic patch also passed. I did not change that transport test or claim the intermittent failure was reproduced on HEAD. Earlier logs remain under target/m4-fix/.
- The first terminal run failed only the new test's startup fence, before Ctrl+L. I corrected the test to wait for the loaded Codex answer; its focused rerun and the complete rerun passed. No product fix was needed for that failure.

Speed, seven alternated pairs

The supplied /tmp/carlos-base/target/release/carlos ran first, then ./target/release/carlos, seven times each. No build, test, or oracle ran beside these pairs. Every run used perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24 --width 160 --height 48. Every run kept 6,001 messages, 39,347 rendered lines, and 6,000 relevant items.

- replay: 47.90 -> 47.14 ms, -1.59%.

- layout: 14.97 -> 14.67 ms, -2.00%.

- draw: 3.50 -> 3.57 ms, +2.00%.

- scroll: 0.70 -> 0.70 ms, +0.00%.

- typing: 0.59 -> 0.60 ms, +1.69%.

- working: 0.59 -> 0.60 ms, +1.69%.

- append: 0.60 -> 0.60 ms, +0.00%.

Every requested median passes the 5% limit. Raw runs and parsed medians: target/m4-fix/perf-{old,new}-1..7.txt and perf-pairs.json. No memory claim is made from the known zero-RSS benchmark field.

Scope and cleanup

The two new terminal tests are the only added edits outside milestone 4's allowed Rust paths and files already touched by the original change: tests/ui/test_input_status.py and tests/ui/test_claude_questions.py. They are the small workflow tests required by AGENTS.md section 7. New Rust tests live in src/tests/; test module bridges and the two new state_* companions use the allowed app paths. Existing tests' status reads now use the effective view helper where they formerly observed the mixed status string; their expected strings were not weakened.

The reference worktrees and their temporary Rust overrides are removed before handoff. The pre-existing /tmp/carlos-base worktree and its release binary remain intact. All temporary main-tree oracle hooks were restored from exact pre-hook backups; no m4_audit, m4_replay, or audit helper remains in src/. Harness code, inputs, full outputs, comparisons, and logs remain under target/m4-fix/. The prior worker/reviewer artifacts were retained.

Unfinished or uncertain

Exact pre-plan full-stream parity remains unmet for the 86 proven committed differences above. I have not hidden them, changed their expected output, or expanded the fix to revert earlier milestones. The fixed-seed corpus and deterministic terminal peers do not prove every possible live backend/thread schedule. This milestone moved no thread boundary. No live model or personal session was used.
