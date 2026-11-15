//! Preserve deliberate launch choices without republishing resumed native defaults.

use serde_json::json;

use super::settings_contract_support::{session, tick};
use crate::protocol_params::ModelInfo;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

fn native_session() -> SessionState {
    let mut session = session();
    session.set_available_models(vec![ModelInfo {
        model: "native".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into(), "high".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]);
    session.set_runtime_settings(Some("native".into()), Some("low".into()), None);
    session.set_codex_future_settings(session.codex_runtime_choices());
    session
}

#[test]
fn resumed_native_settings_are_not_republished_and_unknown_summary_stays_inherited() {
    let mut session = native_session();
    let peer = TestBackend::default();
    session.prepare_codex_startup(true, None);
    session.install_codex_startup();
    assert_eq!(
        session.next_turn_runtime_settings(),
        (Some("native".into()), Some("low".into()), None)
    );
    assert_eq!(session.codex_connection.initial_choices, None);
    session.finish_codex_reconnect(false);
    assert!(!tick(&mut session, &peer));
    assert!(peer
        .calls
        .lock()
        .expect("no resume setting writes")
        .is_empty());
}

#[test]
fn explicit_resume_summary_and_new_thread_defaults_send_only_the_deliberate_summary() {
    for (resumed, summary_override, expected) in [
        (true, Some("detailed".into()), "detailed"),
        (false, None, "concise"),
    ] {
        let mut session = native_session();
        let peer = TestBackend::default();
        session.prepare_codex_startup(resumed, summary_override);
        session.install_codex_startup();
        session.finish_codex_reconnect(false);
        assert!(tick(&mut session, &peer));
        let calls = peer.calls.lock().expect("startup write");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "thread/settings/update");
        assert_eq!(calls[0].1, json!({"threadId":"root", "summary":expected}));
        assert_eq!(session.runtime_settings().current_summary(), &None);
    }
}
