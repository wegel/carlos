//! Keep native context, initialization and interruption notices on the real session reducer.

use super::{feed as handle_notification_line, session};
use crate::session::context_usage::ContextUsage;
use crate::session::models::Role;

#[test]
fn handle_notification_updates_context_usage_when_present() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/completed\",\"params\":{\"usage\":{\"context_window\":256000,\"context_tokens\":128000}}}",
        );

    assert_eq!(app.context_usage(), None);
}

#[test]
fn handle_notification_thread_initialized_updates_thread_id() {
    let mut app = session("claude-pending-session");

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id(), "session-123");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6");
}

#[test]
fn handle_notification_thread_initialized_clears_context_usage_when_thread_changes() {
    let mut app = session("claude-pending-session");
    app.set_context_usage(Some(ContextUsage {
        used: 1_000_000,
        max: 1_000_000,
    }));

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id(), "session-123");
    assert_eq!(app.context_usage(), None);
}

#[test]
fn handle_notification_thread_initialized_preserves_context_usage_for_same_thread() {
    let mut app = session("session-123");
    app.set_context_usage(Some(ContextUsage {
        used: 80_495,
        max: 1_000_000,
    }));

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(
        app.context_usage(),
        Some(ContextUsage {
            used: 80_495,
            max: 1_000_000,
        })
    );
}

#[test]
fn handle_notification_thread_initialized_preserves_existing_effort_when_only_model_arrives() {
    let mut app = session("claude-pending-session");
    app.set_runtime_settings(None, Some("high".to_string()), None);

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id(), "session-123");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6/high");
}

#[test]
fn handle_notification_thread_compacted_clears_context_usage() {
    let mut app = session("thread-1");
    app.set_context_usage(Some(ContextUsage {
        used: 80_495,
        max: 1_000_000,
    }));

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/compacted\",\"params\":{\"threadId\":\"thread-1\"}}",
    );

    assert_eq!(app.context_usage(), None);
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].text, "↻ Context compacted");
}

#[test]
fn handle_notification_turn_completed_interrupted_appends_system_message() {
    let mut app = session("thread-1");
    app.set_active_turn_id(Some("turn-1".to_string()));

    handle_notification_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"interrupted\"}}}",
    );

    assert_eq!(*app.active_turn_id(), None);
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::System);
    assert_eq!(app.messages()[0].text, "Turn interrupted");
}

#[test]
fn append_turn_interrupted_marker_is_deduplicated() {
    let mut app = session("thread-1");
    app.append_turn_interrupted_marker();
    app.append_turn_interrupted_marker();
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::System);
    assert_eq!(app.messages()[0].text, "Turn interrupted");
}

#[test]
fn handle_notification_thread_token_usage_updated_sets_context_usage() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":256000,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":0,\"outputTokens\":0,\"reasoningOutputTokens\":0,\"totalTokens\":0},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":100000,\"outputTokens\":22000,\"reasoningOutputTokens\":0,\"totalTokens\":122000}}}}",
        );

    assert_eq!(
        app.context_usage(),
        Some(ContextUsage {
            used: 122_000,
            max: 256_000
        })
    );
}

#[test]
fn handle_notification_thread_token_usage_prefers_last_when_total_exceeds_window() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":258400,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":46000,\"outputTokens\":300,\"reasoningOutputTokens\":37,\"totalTokens\":46337},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":301000,\"outputTokens\":2104,\"reasoningOutputTokens\":30,\"totalTokens\":303134}}}}",
        );

    assert_eq!(
        app.context_usage(),
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}
