//! Keep prompt labels, quiet defaults and explicit mode feedback separate from native input text.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::json;

use super::codex_question_support::Peer;
use crate::app::claude_notifications::record_submitted_prompt;
use crate::app::context_usage::ContextUsage;
use crate::app::input_events::handle_terminal_event;
use crate::app::input_mode;
use crate::app::models::{MessageKind, Role, TerminalSize};
use crate::app::notifications::handle_server_message_line;
use crate::app::render_status::draw_status_bar;
use crate::app::state::AppState;

fn status(app: &AppState, width: usize) -> String {
    let mut buffer = Buffer::empty(Rect::new(0, 0, width as u16, 3));
    draw_status_bar(&mut buffer, app, TerminalSize { width, height: 32 }, 3);
    (0..width)
        .map(|column| buffer[(column as u16, 1)].symbol())
        .collect()
}

#[test]
fn default_is_quiet_queue_is_visible_and_explicit_steer_notice_expires_without_native_work() {
    for claude in [false, true] {
        let mut app = AppState::new("root".into());
        app.claude.enabled = claude;
        app.codex.enabled = !claude;
        let now = Instant::now();
        assert!(!status(&app, 100).contains("Steer"));
        assert!(!status(&app, 100).contains("F2"));
        assert!(!status(&app, 100).contains("F3"));
        assert!(input_mode::wait(&app, now).is_none());
        input_mode::toggle(&mut app, now);
        assert!(status(&app, 100).contains("Queue"));
        input_mode::toggle(&mut app, now);
        assert!(status(&app, 100).contains("Steer"));
        assert_eq!(input_mode::wait(&app, now), Some(Duration::from_secs(2)));
        assert!(input_mode::tick(&mut app, now + Duration::from_secs(2)));
        assert!(!status(&app, 100).contains("Steer"));
        assert!(input_mode::wait(&app, now + Duration::from_secs(3)).is_none());
    }
}

#[test]
fn narrow_queue_reserves_compact_context_and_keeps_weekly_at_the_right_edge() {
    for claude in [false, true] {
        let mut app = AppState::new("root".into());
        app.claude.enabled = claude;
        app.codex.enabled = !claude;
        app.context_usage = Some(ContextUsage {
            used: 90_000,
            max: 100_000,
        });
        app.claude.context.fresh = true;
        let reset = crate::app::weekly_usage::unix_now() + 3 * 86_400;
        app.rate_limits.update(
            &json!({"secondary":{"usedPercent":50,"windowDurationMins":10080,"resetsAt":reset}}),
        );
        app.claude.quota.event(&json!({"status":"allowed_warning","rateLimitType":"seven_day","utilization":0.5,"resetsAt":reset}));
        input_mode::toggle(&mut app, Instant::now());
        for width in [28, 36, 54] {
            let line = status(&app, width);
            assert!(line.contains("Queue"), "{width}: {line}");
            assert!(line.contains("90%"), "{width}: {line}");
            assert!(line.trim_end().ends_with("W50%/3.0d"), "{width}: {line}");
        }
    }
}

#[test]
fn captured_claude_uuid_kinds_survive_later_mode_changes_and_do_not_change_text_or_editability() {
    let mut app = AppState::new("root".into());
    app.claude.enabled = true;
    for (id, kind) in [
        ("normal", "prompt"),
        ("steer", "steer"),
        ("queued", "prompt"),
    ] {
        let index = app.append_message(Role::User, "identical 日本語");
        app.record_input_history("identical 日本語", Some(index));
        app.claude.prompts.queue = kind == "steer";
        record_submitted_prompt(
            &mut app,
            &json!({"result":{"turn":{"id":id},"inputKind":kind}}),
        );
    }
    assert_eq!(
        app.messages
            .iter()
            .map(|message| message.kind)
            .collect::<Vec<_>>(),
        vec![
            MessageKind::Plain,
            MessageKind::UserSteer,
            MessageKind::Plain
        ]
    );
    assert!(app
        .messages
        .iter()
        .all(|message| message.text == "identical 日本語"));
    app.enter_rewind_mode();
    assert_eq!(app.rewind_selected_turn_id(), Some("queued"));
    assert!(app.navigate_input_history_up());
    assert_eq!(app.rewind_selected_turn_id(), Some("steer"));
    assert!(
        app.input_history.selected_native_request().is_none(),
        "Claude never uses the Codex steer-refusal ledger"
    );
}

#[test]
fn loaded_classification_never_promotes_unknown_or_automatic_records_to_human() {
    let mut app = AppState::new("root".into());
    app.claude.enabled = true;
    let records: Vec<_> = [("human",Some("prompt")),("human",Some("steer")),("human",None),("automatic",Some("steer")),("unknown",Some("steer"))].into_iter().enumerate().map(|(index,(source,kind))|
        json!({"source":source,"inputKind":kind,"message":{"type":"user","uuid":format!("id-{index}"),"session_id":"root","message":{"role":"user","content":"same"}}})).collect();
    let _ = handle_server_message_line(&mut app,&json!({"method":"claude/controller","params":{"requestMethod":"sessions.read","frame":{"type":"reply","ok":true,"result":{"messages":records,"nextOffset":null}}}}).to_string());
    assert_eq!(app.input_history_len(), 3);
    assert_eq!(app.messages[0].kind, MessageKind::Plain);
    assert_eq!(app.messages[1].kind, MessageKind::UserSteer);
    assert_eq!(app.messages[2].kind, MessageKind::Plain);
    assert!(app.messages[3..]
        .iter()
        .all(|message| message.role == Role::System && message.kind != MessageKind::UserSteer));
}

#[test]
fn narrow_help_end_then_up_and_pageup_moves_the_actual_visible_lines() {
    let peer = Peer::default();
    let mut app = AppState::new("root".into());
    app.viewport.show_help = true;
    let size = TerminalSize {
        width: 28,
        height: 12,
    };
    let key = |app: &mut AppState, code| {
        handle_terminal_event(
            &peer,
            app,
            Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            size,
        );
    };
    key(&mut app, KeyCode::End);
    let maximum = crate::app::overlay_render::help_scroll_limit(size);
    assert_eq!(app.viewport.help_scroll, maximum);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.viewport.help_scroll, maximum - 1);
    key(&mut app, KeyCode::PageUp);
    assert_eq!(app.viewport.help_scroll, maximum.saturating_sub(6));
    key(&mut app, KeyCode::Home);
    assert_eq!(app.viewport.help_scroll, 0);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 28, 12));
    crate::app::overlay_render::draw_help_overlay(&mut buffer, size, &app);
    let screen = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(screen.contains("F2 tasks"));
    assert!(screen.contains("F3 switches"));
}

#[test]
fn narrow_default_mode_keeps_recovery_error_and_pending_notices_beside_quota_and_percent() {
    let mut app = AppState::new("root".into());
    app.codex.enabled = true;
    app.context_usage = Some(ContextUsage {
        used: 90_000,
        max: 100_000,
    });
    app.rate_limits.update(&json!({"secondary":{"usedPercent":50,"windowDurationMins":10080,"resetsAt":crate::app::weekly_usage::unix_now()+3*86400}}));
    for (kind, full, compact) in [
        ("draft", "Unsent draft", "Draft"),
        ("error", "Queue error", "Q!"),
        ("pending", "Queue pending", "Q…"),
    ] {
        app.codex.queue.recovery = (kind == "draft").then(|| "retained draft".into());
        app.codex.queue.error = (kind == "error").then(|| "native rejected it".into());
        app.codex.queue.operation =
            (kind == "pending").then(|| crate::app::codex_queue_state::QueueOperation {
                token: 1,
                method: "thread/queue/add".into(),
                params: json!({}),
                submitted: None,
            });
        for width in [28, 36] {
            let line = status(&app, width);
            assert!(
                line.contains(if width < 32 { compact } else { full }),
                "{width}: {line}"
            );
            assert!(line.contains("90%"), "{width}: {line}");
            assert!(line.trim_end().ends_with("W50%/3.0d"), "{width}: {line}");
            assert!(!line.contains("Steer"));
        }
    }
}
