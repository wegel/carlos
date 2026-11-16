//! Reject malformed ACP choices and preserve exact card-local selection mapping.

use carlos_proto::session::{ApprovalInput, Backend, Decision};
use serde_json::{json, Value};

use super::{answer, input};
use crate::session::approval_state::ApprovalChoice;
use crate::session::state::SessionState;

fn request() -> Value {
    json!({"id":"7","params":{"sessionId":"root","toolCall":{"title":"Native command"},"options":[
        {"optionId":"PRIVATE_ALLOW","name":"Allow once","kind":"allow_once"},
        {"optionId":"PRIVATE_DENY","name":"Reject always","kind":"reject_always"}
    ]}})
}

fn state() -> SessionState {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.set_client_identity(Backend::Grok, "host".into());
    state.receive_grok_permission(
        "carlos/grok/request",
        &json!({"token":"private-token","request":request()}),
    );
    state
}

#[test]
fn unsupported_options_never_offer_an_allow_or_free_text_fallback() {
    let mut malformed = Vec::new();
    malformed.push(json!([]));
    malformed.push(Value::Null);
    let options = request()["params"]["options"].clone();
    for (field, value) in [
        ("kind", json!("future_kind")),
        ("optionId", json!(null)),
        ("name", json!("")),
        ("optionId", json!("PRIVATE_DENY")),
    ] {
        let mut changed = options.clone();
        changed[0][field] = value;
        malformed.push(changed);
    }
    for options in malformed {
        let mut changed = request();
        changed["params"]["options"] = options;
        assert!(matches!(input(&changed), ApprovalInput::Unsupported { .. }));
        let mut state = SessionState::new("root".into(), Default::default(), false);
        state.set_client_identity(Backend::Grok, "host".into());
        state.receive_grok_permission(
            "carlos/grok/request",
            &json!({"token":"token","request":changed}),
        );
        let card = &state.export_approvals()[0];
        assert_eq!(card.choices, [Decision::Cancel]);
    }
}

#[test]
fn only_one_offered_card_local_value_can_select_an_exact_private_native_option() {
    let state = state();
    let card = &state.export_approvals()[0];
    let facts = state.approval().pending().expect("live card").facts();
    let serialized = serde_json::to_string(card).expect("public card");
    assert!(!serialized.contains("PRIVATE_"));
    assert!(!serialized.contains("private-token"));
    assert_eq!(card.display_id.as_deref(), Some("\"7\""));
    for value in [
        json!({"fields":[]}),
        json!({"fields":[{"key":"wrong","values":[0]}]}),
        json!({"fields":[{"key":"permission","values":["PRIVATE_ALLOW"]}]}),
        json!({"fields":[{"key":"permission","values":[]}]}),
        json!({"fields":[{"key":"permission","values":[0,1]}]}),
        json!({"fields":[{"key":"permission","values":[0],"text":"extra"}]}),
        json!({"fields":[{"key":"permission","values":[2]}]}),
    ] {
        assert!(answer(facts, card, &value).is_err(), "must reject {value}");
    }
    let (choice, body) = answer(
        facts,
        card,
        &json!({"fields":[{"key":"permission","values":[1]}]}),
    )
    .expect("offered rejection");
    assert_eq!(choice, ApprovalChoice::Decline);
    assert_eq!(
        body,
        json!({"outcome":{"outcome":"selected","optionId":"PRIVATE_DENY"}})
    );
}

#[test]
fn repeated_delivery_preserves_card_identity_and_retiring_one_queue_entry_keeps_the_rest() {
    let mut state = state();
    let first = state.export_approvals()[0].id.clone();
    state.receive_grok_permission(
        "carlos/grok/request",
        &json!({"token":"private-token","request":request()}),
    );
    assert_eq!(state.export_approvals().len(), 1);
    assert_eq!(state.export_approvals()[0].id, first);
    for token in ["second", "third"] {
        state.receive_grok_permission(
            "carlos/grok/request",
            &json!({"token":token,"request":request()}),
        );
    }
    state.retire_grok_permission(&json!("second"));
    assert_eq!(state.export_approvals().len(), 2);
    state.retire_grok_permission(&json!("private-token"));
    assert_ne!(state.export_approvals()[0].id, first);
    state.clear_grok_permissions();
    assert!(state.export_approvals().is_empty());
}
