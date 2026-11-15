//! Preserve bounded native labels, freshness and Ralph guards in numbered header facts.

use carlos_proto::events::Event;
use carlos_proto::session::Backend;
use carlos_proto::status::MAX_STATUS_LABEL_BYTES;
use carlos_proto::task_overview::TaskMainState;
use serde_json::json;

use super::status_label;
use crate::session::ralph::RalphConfig;
use crate::session::state::SessionState;

#[test]
fn transient_labels_keep_short_text_and_clip_utf8_with_a_visible_ellipsis() {
    for value in [
        "",
        "Requesting",
        "日本語",
        &"x".repeat(MAX_STATUS_LABEL_BYTES),
    ] {
        let label = status_label(value);
        assert_eq!(label.text, value);
        assert_eq!(label.bytes.get(), value.len() as u64);
        assert!(label.complete);
    }
    for value in ["x".repeat(3000), "界".repeat(1000), "🦀".repeat(1000)] {
        let label = status_label(&value);
        assert!(label.text.len() <= MAX_STATUS_LABEL_BYTES);
        assert!(label.text.ends_with('…'));
        assert!(value.starts_with(label.text.trim_end_matches('…')));
        assert_eq!(label.bytes.get(), value.len() as u64);
        assert!(!label.complete);
    }
}

#[test]
fn main_activity_changes_publish_without_a_task_panel_or_transcript_change() {
    let mut session = session();
    session.publish();
    session.record_claude_activity(&json!({"state":"running"}));
    session.record_claude_thinking(&json!({"estimated_tokens":43}));
    session.record_claude_status(&json!({"label":"Requesting"}));
    let published = session.publish();
    let info = published
        .records()
        .iter()
        .filter_map(|record| published.to_wire(record))
        .find_map(|record| match record.event {
            Event::InfoChanged { info } => Some(info),
            _ => None,
        })
        .expect("numbered activity facts");
    let status = info.status.expect("status facts");
    let activity = status.activity.expect("Claude activity");
    assert_eq!(activity.state, TaskMainState::Running);
    assert_eq!(activity.thinking_tokens, Some(43));
    assert_eq!(activity.transient.expect("native label").text, "Requesting");
    assert!(!status.context_fresh);
    assert!(
        session.publish().records().is_empty(),
        "unchanged facts stay quiet"
    );
}

#[test]
fn only_the_reported_backend_supplies_weekly_quota() {
    let mut session = session();
    let reset = crate::session::usage::unix_now() + 86400;
    session.update_rate_limits(&json!({"secondary":{
        "usedPercent":90,"windowDurationMins":10080,"resetsAt":reset}}));
    assert_eq!(
        session.event_quota(),
        None,
        "Claude cannot borrow Codex quota"
    );
    session.claude_quota_event(&json!({"rateLimitType":"seven_day",
        "utilization":0.4,"resetsAt":reset}));
    let quota = session.event_quota().expect("Claude weekly allowance");
    assert_eq!(quota.remaining_pct, 60.0);
    assert_eq!(quota.resets_at, Some(reset));
    session.set_client_identity(Backend::Grok, "host".into());
    assert_eq!(
        session.event_quota(),
        None,
        "Grok has no shared weekly bucket"
    );
    session.set_client_identity(Backend::Codex, "host".into());
    assert_eq!(
        session
            .event_quota()
            .expect("Codex allowance")
            .remaining_pct,
        10.0
    );
}

#[test]
fn ralph_flags_preserve_pending_work_without_exporting_queued_prompt_text() {
    let mut session = session();
    session.enable_ralph_mode(RalphConfig {
        prompt_path: "private-path".into(),
        base_prompt: "private base".into(),
        done_marker: "done".into(),
        blocked_marker: "blocked".into(),
        waiting_marker: "waiting".into(),
        continuation_prompt: "private automatic text".into(),
    });
    session.queue_ralph_continuation("private automatic text");
    session
        .ralph_request_toggle(true)
        .expect("defer existing toggle");
    let status = session.export_status();
    assert!(status.ralph.toggle_pending);
    assert!(status.ralph.continuation_pending);
    assert!(!status.ralph.waiting_for_user);
    let serialized = serde_json::to_string(&status).expect("typed status");
    assert!(!serialized.contains("private"));
    session.disable_ralph_mode();
    let status = session.export_status();
    assert!(!status.ralph.toggle_pending);
    assert!(!status.ralph.continuation_pending);
}

fn session() -> SessionState {
    let mut session = SessionState::new("native".into(), Default::default(), false);
    session.enable_claude();
    session.set_client_identity(Backend::Claude, "host".into());
    session
}
