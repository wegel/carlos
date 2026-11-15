//! Keep native request captions bounded and distinct from the opaque addresses used by commands.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::json;

use super::request_label;
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn native_request_captions_preserve_json_type_and_bound_escaped_unicode_bytes() {
    assert_eq!(request_label(&json!(9)), "9");
    assert_eq!(request_label(&json!("9")), "\"9\"");
    let label = request_label(&json!("🌍\u{1b}[2J\u{202e}".repeat(3000)));
    assert!(label.len() <= 4096);
    assert!(label.ends_with('…'));
    assert!(!label.chars().any(char::is_control));
    assert!(!label.contains('\u{202e}'));
}

#[test]
fn a_native_request_caption_shaped_like_an_opaque_id_cannot_address_the_card() {
    let native = format!("{}-{:016x}", "a".repeat(32), 7);
    let foreign: carlos_proto::ids::EntryId =
        native.parse().expect("valid opaque-shaped native label");
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.receive_claude_callback(&json!({"callbackId":native,"kind":"permission","payload":{"toolName":"Read","input":{"file_path":"a.rs"}}}));
    let card = session.export_approvals()[0].clone();
    assert_ne!(card.id, foreign);
    assert_eq!(
        card.display_id.as_deref(),
        Some(format!("\"{native}\"").as_str())
    );
    let backend = TestBackend::default();
    assert!(session
        .prepare_engine_answer(
            &backend,
            EntryId::from_wire(&foreign),
            ApprovalAnswer::Decision {
                decision: Decision::Once
            }
        )
        .is_err());
    session
        .prepare_engine_answer(
            &backend,
            EntryId::from_wire(&card.id),
            ApprovalAnswer::Decision {
                decision: Decision::Once,
            },
        )
        .expect("only the actual opaque card address is accepted");
}
