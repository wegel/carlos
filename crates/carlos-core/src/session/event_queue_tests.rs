//! Preserve shared queue bounds and scope while keeping private edits out of published facts.

use carlos_proto::events::Event;
use carlos_proto::queue::{QueueChange, MAX_DISPLAY_ID_BYTES};
use serde_json::{json, Value};

use super::event_tests::synchronize;
use super::state::SessionState;

fn session() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core.codex_queue.known = true;
    core.codex_queue.entries = (0..40).map(|index| json!({"id":format!("native-{index}"),
        "clientUserMessageId":"human", "input":[{"type":"text", "text":format!("row {index} 🌍"), "text_elements":[]}]})).collect();
    core
}

#[test]
fn queue_pages_share_one_revision_but_no_selection_and_publish_only_shared_read_errors() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    let page = core.queue_page(0, 1000);
    assert_eq!(page.entries.len(), 32);
    assert!(page.has_more);
    let other = core.queue_page(32, 32);
    assert_eq!(other.entries.len(), 8);
    assert_eq!(other.queue.revision, page.queue.revision);
    assert!(!other.has_more);
    let wire = serde_json::to_string(&page).expect("queue page");
    assert!(wire.contains("\"display_id\":\"native-0\""));
    let command = serde_json::to_string(&QueueChange::Delete {
        entry: page.entries[0].id.clone(),
    })
    .expect("opaque command");
    assert!(!command.contains("native-"));
    assert!(!wire.contains("threadId"));
    core.codex_queue.error = Some("PRIVATE QUEUE ERROR".into());
    assert!(core.publish().records().is_empty());
    core.codex_queue.read_error = Some("shared read failure".into());
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|event| matches!(event.event, Event::QueueChanged { .. })));
    assert_eq!(
        client.queue.expect("shared facts").read_error.as_deref(),
        Some("shared read failure")
    );
}

#[test]
fn queue_display_ids_are_bounded_sanitized_and_never_resolve_native_targets() {
    let mut core = session();
    let native = "b".repeat(64);
    core.codex_queue.entries[0]["id"] = json!(native);
    core.codex_queue.entries[1]["id"] = json!("🌍\n\x1b\u{202e}".repeat(1000));
    core.publish();
    let page = core.queue_page(0, 32);
    assert_eq!(page.entries[0].display_id.as_deref(), Some(native.as_str()));
    assert_ne!(page.entries[0].id.as_str(), native);
    assert!(core
        .queue_entry(&native.parse().expect("valid opaque shape"))
        .is_none());
    assert!(core.queue_entry(&page.entries[0].id).is_some());
    let label = page.entries[1]
        .display_id
        .as_deref()
        .expect("display label");
    assert!(label.len() <= MAX_DISPLAY_ID_BYTES);
    assert!(label.ends_with('…'));
    assert!(!label.chars().any(|ch| ch.is_control() || ch == '\u{202e}'));
    assert!(label.contains("\\n") && label.contains("\\u{1b}"));
}

#[test]
fn queue_ids_survive_input_changes_but_retire_with_their_native_root_or_connection() {
    let mut core = session();
    let entry = core.queue_entry_id(&core.codex_queue.entries[0]);
    let revision = core.queue_revision();
    core.codex_queue.entries[0]["input"][0]["text"] = json!("changed input");
    core.codex_queue.invalidate();
    assert_eq!(core.queue_entry_id(&core.codex_queue.entries[0]), entry);
    assert_ne!(core.queue_revision(), revision);
    core.advance_codex_epoch();
    assert!(core.queue_entry(&entry).is_none());
    let replacement = core.queue_entry_id(&core.codex_queue.entries[0]);
    core.adopt_native_thread("fork".into());
    assert!(core.queue_entry(&replacement).is_none());
    core.codex_queue.revision = u64::MAX;
    core.codex_queue.known = true;
    core.codex_queue.invalidate();
    assert!(core.codex_queue.exhausted);
    assert!(!core.codex_queue.known);
}

#[test]
fn queue_previews_bound_large_text_and_unknown_attachment_fields_before_cloning() {
    let text = "\0🌍".repeat(1024 * 1024);
    for input in [
        json!([{"type":"text", "text":text}]),
        json!([{"type":"future_attachment", "payload":text}]),
    ] {
        let (preview, complete) = super::queue_preview::preview(&input);
        assert!(!complete);
        assert_eq!(preview.chars().count(), 1024);
        assert!(
            serde_json::to_string(&preview)
                .expect("escaped preview")
                .len()
                <= 6146
        );
    }
    for (input, expected) in [
        (json!([]), ""),
        (
            json!([{"text":"first"}, {"text":"second"}]),
            "first\nsecond",
        ),
    ] {
        assert_eq!(
            super::queue_preview::preview(&input),
            (expected.into(), true)
        );
    }
    let mut core = session();
    let rich: Value = json!({"id":"rich", "clientUserMessageId":"human", "input":[{"type":"text", "text":"rich", "text_elements":[{"type":"future"}]}]});
    core.codex_queue.entries = vec![rich.clone()];
    core.publish();
    let page = core.queue_page(0, 32);
    assert_eq!(page.entries[0].preview, "rich");
    assert!(page.entries[0].text_bytes.is_none());
    assert_eq!(
        core.codex_queue.entries[0], rich,
        "preview never rewrites rich native input"
    );
}
