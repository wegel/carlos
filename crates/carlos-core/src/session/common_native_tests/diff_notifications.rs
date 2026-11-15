//! Ignore raw function echoes and retain commentary phase on actual native item events.

use super::{feed as handle_notification_line, session};
use crate::session::models::Role;

#[test]
fn raw_function_call_is_ignored_before_command_execution_item_started() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_4\",\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_4\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages().len(), 1);
}

#[test]
fn item_started_agent_message_uses_commentary_role_when_phase_present() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"agentMessage\",\"id\":\"msg-1\",\"phase\":\"commentary\",\"text\":\"\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/agentMessage/delta\",\"params\":{\"itemId\":\"msg-1\",\"delta\":\"checking\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::Commentary);
    assert_eq!(app.messages()[0].text, "checking");
}
