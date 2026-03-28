use serde_json::json;

use super::{
    append_history_from_thread, build_rendered_lines, handle_notification_line,
    handle_server_message_line, load_history_from_start_or_resume, AppState, ContextUsage,
    MessageKind, Role, ServerRequestAction,
};

#[test]
fn handle_notification_updates_context_usage_when_present() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/completed\",\"params\":{\"usage\":{\"context_window\":256000,\"context_tokens\":128000}}}",
        );

    assert_eq!(app.context_usage, None);
}

#[test]
fn handle_notification_turn_completed_interrupted_appends_system_message() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    handle_notification_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"interrupted\"}}}",
    );

    assert_eq!(app.active_turn_id, None);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn incoming_agent_delta_does_not_reenable_auto_follow_when_scrolled_up() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Assistant, "hello");
    app.put_agent_item_mapping("item-1", idx);
    app.viewport.auto_follow_bottom = false;
    app.viewport.scroll_top = 4;

    handle_notification_line(
        &mut app,
        "{\"method\":\"item/agentMessage/delta\",\"params\":{\"itemId\":\"item-1\",\"delta\":\" world\"}}",
    );

    assert!(!app.viewport.auto_follow_bottom);
    assert_eq!(app.viewport.scroll_top, 4);
    assert_eq!(app.messages[idx].text, "hello world");
}

#[test]
fn handle_server_request_command_execution_sets_pending_approval() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"itemId\":\"item-1\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"command\":\"git diff -- src/main.rs\",\"cwd\":\"/repo\",\"reason\":\"needs write access\",\"availableDecisions\":[\"accept\",\"acceptForSession\",\"decline\",\"cancel\"]}}",
    );

    assert!(action.is_none());
    let pending = app.approval.pending.expect("pending approval");
    assert_eq!(pending.method, "item/commandExecution/requestApproval");
    assert_eq!(pending.title, "Approve command execution");
    assert_eq!(pending.detail_lines[0], "git diff -- src/main.rs");
    assert!(pending.can_accept_for_session);
    assert!(pending.can_decline);
    assert!(pending.can_cancel);
}

#[test]
fn permissions_approval_response_allows_turn_or_session_grant() {
    let request = super::state::PendingApprovalRequest {
        request_id: json!("req-2"),
        method: "item/permissions/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines: vec!["network access".to_string()],
        requested_permissions: Some(json!({"network":{"enabled":true}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Accept),
        Some(json!({"permissions":{"network":{"enabled":true}}}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::AcceptForSession),
        Some(json!({"permissions":{"network":{"enabled":true}},"scope":"session"}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Decline),
        Some(json!({"permissions":{}}))
    );
}

#[test]
fn unsupported_server_request_returns_jsonrpc_error_action() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"item/tool/requestUserInput\",\"params\":{\"itemId\":\"item-1\"}}",
    );

    match action {
        Some(ServerRequestAction::ReplyError {
            request_id,
            code,
            message,
        }) => {
            assert_eq!(request_id, json!(99));
            assert_eq!(code, -32601);
            assert!(message.contains("unsupported server request"));
        }
        _ => panic!("expected reply error"),
    }
    assert!(app.approval.pending.is_none());
}

#[test]
fn append_turn_interrupted_marker_is_deduplicated() {
    let mut app = AppState::new("thread-1".to_string());
    app.append_turn_interrupted_marker();
    app.append_turn_interrupted_marker();
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn handle_notification_thread_token_usage_updated_sets_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":256000,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":0,\"outputTokens\":0,\"reasoningOutputTokens\":0,\"totalTokens\":0},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":100000,\"outputTokens\":22000,\"reasoningOutputTokens\":0,\"totalTokens\":122000}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 122_000,
            max: 256_000
        })
    );
}

#[test]
fn handle_notification_thread_token_usage_prefers_last_when_total_exceeds_window() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":258400,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":46000,\"outputTokens\":300,\"reasoningOutputTokens\":37,\"totalTokens\":46337},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":301000,\"outputTokens\":2104,\"reasoningOutputTokens\":30,\"totalTokens\":303134}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_codex_event_token_count_sets_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":{\"total_token_usage\":{\"input_tokens\":8815,\"cached_input_tokens\":0,\"output_tokens\":41,\"reasoning_output_tokens\":0,\"total_tokens\":8856},\"last_token_usage\":{\"input_tokens\":8815,\"cached_input_tokens\":0,\"output_tokens\":41,\"reasoning_output_tokens\":0,\"total_tokens\":8856},\"model_context_window\":258400,\"cost_usd\":0.0045096,\"total_cost_usd\":0.0045096}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 8_856,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_token_count_prefers_last_when_total_exceeds_window() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":{\"total_token_usage\":{\"input_tokens\":301030,\"cached_input_tokens\":264832,\"output_tokens\":2104,\"reasoning_output_tokens\":906,\"total_tokens\":303134},\"last_token_usage\":{\"input_tokens\":46249,\"cached_input_tokens\":43648,\"output_tokens\":88,\"reasoning_output_tokens\":56,\"total_tokens\":46337},\"model_context_window\":258400}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_codex_event_token_count_ignores_null_info() {
    let mut app = AppState::new("thread-1".to_string());
    app.context_usage = Some(ContextUsage {
        used: 10_000,
        max: 256_000,
    });
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":null}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 10_000,
            max: 256_000
        })
    );
}

#[test]
fn load_history_does_not_seed_context_usage_from_start_response() {
    let mut app = AppState::new("thread-1".to_string());
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": {
                "id": "thread-1",
                "tokenUsage": {
                    "modelContextWindow": 256000,
                    "last": {
                        "cachedInputTokens": 0,
                        "inputTokens": 0,
                        "outputTokens": 0,
                        "reasoningOutputTokens": 0,
                        "totalTokens": 0
                    },
                    "total": {
                        "cachedInputTokens": 0,
                        "inputTokens": 80000,
                        "outputTokens": 12000,
                        "reasoningOutputTokens": 0,
                        "totalTokens": 92000
                    }
                },
                "turns": []
            }
        }
    })
    .to_string();

    load_history_from_start_or_resume(&mut app, &response).expect("load history");
    assert_eq!(app.context_usage, None);
}

#[test]
fn load_history_seeds_input_history_from_user_messages() {
    let mut app = AppState::new("thread-1".to_string());
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": {
                "id": "thread-1",
                "turns": [
                    {
                        "items": [
                            {
                                "type": "userMessage",
                                "content": [
                                    {"type": "text", "text": "first message"}
                                ]
                            },
                            {
                                "type": "agentMessage",
                                "text": "reply"
                            },
                            {
                                "type": "userMessage",
                                "content": [
                                    {"type": "text", "text": "second message"}
                                ]
                            }
                        ]
                    }
                ]
            }
        }
    })
    .to_string();

    load_history_from_start_or_resume(&mut app, &response).expect("load history");
    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "second message");
    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "first message");
}

#[test]
fn handle_notification_thread_compacted_appends_system_marker() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/compacted\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn handle_notification_item_completed_context_compaction_appends_marker() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"contextCompaction\",\"id\":\"ctx-1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn append_history_from_thread_includes_context_compaction_marker() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "contextCompaction",
                        "id": "ctx-1"
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn append_history_from_thread_preserves_agent_commentary_phase() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "agentMessage",
                        "id": "a-1",
                        "phase": "commentary",
                        "text": "checking the diff"
                    },
                    {
                        "type": "agentMessage",
                        "id": "a-2",
                        "phase": "final",
                        "text": "done"
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, Role::Commentary);
    assert_eq!(app.messages[0].text, "checking the diff");
    assert_eq!(app.messages[1].role, Role::Assistant);
    assert_eq!(app.messages[1].text, "done");
}

#[test]
fn append_history_from_thread_reads_reasoning_summary_text_objects() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "reasoning",
                        "id": "r-1",
                        "summary": [
                            { "type": "summary_text", "text": "**Plan**\n\nParagraph" },
                            { "type": "summary_text", "text": "**Check**" }
                        ]
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Reasoning);
    assert_eq!(app.messages[0].text, "**Plan**\n\nParagraph\n**Check**");
}

#[test]
fn handle_notification_turn_diff_updated_upserts_diff_message() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));

    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+newer\\n\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].text.contains("+newer"));
}

#[test]
fn handle_notification_item_completed_reasoning_replaces_live_delta_with_summary_objects() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"reasoning\",\"id\":\"reason-1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    app.upsert_reasoning_summary_delta("reason-1", "**Designing fullscreen focus overrides ");
    app.upsert_reasoning_summary_delta("reason-1", "**");

    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"reasoning\",\"id\":\"reason-1\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"**Designing fullscreen focus overrides**\\n\\nI’m working out a helper function.\"},{\"type\":\"summary_text\",\"text\":\"**Analyzing input handling conditions**\"}]},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(
        app.messages[0].text,
        "**Designing fullscreen focus overrides**\n\nI’m working out a helper function.\n**Analyzing input handling conditions**"
    );

    let rendered = build_rendered_lines(&app.messages, 120);
    assert_eq!(rendered[0].text, "Designing fullscreen focus overrides");
    assert_eq!(rendered[1].text, "I’m working out a helper function.");
    assert_eq!(rendered[2].text, "Analyzing input handling conditions");
}

#[test]
fn handle_notification_codex_event_turn_diff_adds_diff_message() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/turn_diff\",\"params\":{\"id\":\"turn-2\",\"msg\":{\"type\":\"turn_diff\",\"unified_diff\":\"diff --git a/a b/a\\n@@ -1 +1 @@\\n-a\\n+b\\n\"}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+b"));
}

#[test]
fn handle_notification_raw_function_call_renders_tool_call() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_1\",\"arguments\":\"{\\\"cmd\\\":\\\"cat test.txt\\\"}\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolCall);
    assert!(app.messages[0].text.contains("run `cat test.txt`"));
}

#[test]
fn handle_notification_raw_function_call_output_updates_existing_call() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_2\",\"arguments\":\"{\\\"cmd\\\":\\\"echo hi\\\"}\"}}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call_output\",\"call_id\":\"call_2\",\"output\":\"hi\\n\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Plain);
    assert!(app.messages[0].text.contains("hi"));
}

#[test]
fn handle_notification_raw_function_call_output_diff_is_rendered_as_diff() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call_output\",\"call_id\":\"call_3\",\"output\":\"diff --git a/x b/x\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));
}

#[test]
fn raw_function_call_dedupes_with_command_execution_item_started() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_4\",\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_4\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
}

#[test]
fn item_started_agent_message_uses_commentary_role_when_phase_present() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"agentMessage\",\"id\":\"msg-1\",\"phase\":\"commentary\",\"text\":\"\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Commentary);
}
