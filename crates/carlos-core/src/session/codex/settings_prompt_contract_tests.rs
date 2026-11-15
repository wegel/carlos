//! Keep prompt inheritance separate from rejected, pending or unsupported native choices.

use serde_json::json;

use super::settings::SettingsEffect;
use super::settings_contract_support::{receive, reply, save, send, session};
use crate::protocol_params::ModelInfo;
use crate::session::test_backend::TestBackend;

#[test]
fn ordinary_prompts_inherit_native_choices_during_pending_apply_and_external_changes() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_runtime_settings(Some("original".into()), Some("low".into()), None);
    session.queue_runtime_settings(Some("requested".into()), Some("high".into()), None);
    save(&mut session, &peer);
    let first = send(&mut session, "while pending");
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("original")
    );
    assert_eq!(
        session.runtime_settings().pending_model().as_deref(),
        Some("requested")
    );
    receive(
        &mut session,
        json!({"method":"thread/settings/updated","params":{"threadId":"root",
        "threadSettings":{"model":"external","effort":"low"}}}),
    );
    let second = send(&mut session, "external native defaults");
    receive(&mut session, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("external")
    );
    for params in [first, second] {
        for key in ["model", "effort", "summary"] {
            assert!(params.get(key).is_none());
        }
    }
}

#[test]
fn native_validation_rejection_keeps_error_data_and_editor_draft_out_of_subsequent_prompts() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_runtime_settings(Some("last-valid".into()), Some("low".into()), None);
    session.queue_runtime_settings(Some("rejected".into()), Some("high".into()), None);
    save(&mut session, &peer);
    let rejected = receive(
        &mut session,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32600,
        "message":"Managed model policy rejected this choice","data":{"policy":"managed"}}}),
        ),
    );
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("last-valid")
    );
    assert!(session.runtime_settings().pending_model().is_none());
    assert!(!session.codex_settings().legacy_overrides());
    assert_eq!(session.codex_settings().errors()["future"]["code"], -32600);
    assert_eq!(
        session.codex_settings().errors()["future"]["data"]["policy"],
        "managed"
    );
    assert!(rejected.effects.iter().any(|effect| matches!(effect,
        SettingsEffect::Rejected { desired, .. } if desired["model"] == "rejected" && desired["effort"] == "high")));
    assert!(send(&mut session, "use valid native settings")
        .get("model")
        .is_none());
}

#[test]
fn only_definitively_unsupported_native_settings_allow_direct_prompt_overrides() {
    let mut session = session();
    let peer = TestBackend::default();
    session.queue_runtime_settings(Some("explicit-compat".into()), Some("low".into()), None);
    save(&mut session, &peer);
    receive(
        &mut session,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Method not found"}}),
        ),
    );
    let sent = send(&mut session, "older native");
    assert_eq!(sent["model"], "explicit-compat");
    assert_eq!(sent["effort"], "low");
}

#[test]
fn a_native_model_without_effort_choices_never_gets_an_invented_effort_override() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_available_models(vec![ModelInfo {
        model: "native-no-effort".into(),
        display_name: "Native".into(),
        supported_efforts: Vec::new(),
        default_effort: None,
        is_default: true,
    }]);
    session.set_runtime_settings(
        Some("native-no-effort".into()),
        Some("old-effort".into()),
        None,
    );
    save(&mut session, &peer);
    assert!(peer.calls.lock().expect("native settings")[0]
        .1
        .get("effort")
        .is_none());
    receive(
        &mut session,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Method not found"}}),
        ),
    );
    assert!(send(&mut session, "no forced effort")
        .get("effort")
        .is_none());
}
