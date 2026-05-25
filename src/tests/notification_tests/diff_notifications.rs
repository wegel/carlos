// Tests for diff-style notification rendering and chunking.

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
