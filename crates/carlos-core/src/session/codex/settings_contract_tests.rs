//! Preserve native future/live choices, validation errors and old-epoch settings replies.

use serde_json::json;

use super::settings;
use super::settings_contract_support::{notice, receive, reply, save, session, tick};
use crate::protocol_params::ModelInfo;
use crate::session::test_backend::TestBackend;

#[test]
fn future_success_and_live_failure_are_separate_and_do_not_relabel_the_running_request() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_runtime_settings(
        Some("old".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    session.set_active_turn_id(Some("active".into()));
    settings::started(&mut session, "active");
    session.queue_runtime_settings(Some("new".into()), Some("low".into()), Some("none".into()));
    save(&mut session, &peer);
    assert_eq!(
        peer.calls
            .lock()
            .expect("native methods")
            .iter()
            .map(|row| row.0.clone())
            .collect::<Vec<_>>(),
        ["thread/settings/update", "turn/settings/update"]
    );
    assert_eq!(
        peer.calls.lock().expect("native scopes")[1].1["turnId"],
        "active"
    );
    receive(
        &mut session,
        reply(
            &peer,
            1,
            json!({"error":{"code":-32601,"message":"method not found"}}),
        ),
    );
    assert!(session.codex_settings().pending());
    let settled = receive(&mut session, reply(&peer, 0, json!({"result":{}})));
    assert!(!session.codex_settings().pending());
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("new")
    );
    assert!(settings::label(&session).starts_with("old/high/concise → future new/low/none"));
    assert!(notice(&settled, "active turn unchanged"));
    assert!(session.runtime_settings().pending_model().is_none());
}

#[test]
fn a_newer_external_native_setting_wins_over_an_older_empty_acknowledgement() {
    let mut session = session();
    let peer = TestBackend::default();
    session.queue_runtime_settings(
        Some("requested".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    save(&mut session, &peer);
    receive(
        &mut session,
        json!({"method":"thread/settings/updated","params":{"threadId":"root",
        "threadSettings":{"model":"external-newer","effort":"low","summary":null,"unknown":"kept"}}}),
    );
    receive(&mut session, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(
        session.next_turn_runtime_settings(),
        (Some("external-newer".into()), Some("low".into()), None)
    );
    assert_eq!(session.codex_settings().future()["unknown"], "kept");
}

#[test]
fn later_local_choices_wait_without_losing_their_values_and_old_epoch_replies_are_ignored() {
    let mut session = session();
    let peer = TestBackend::default();
    session.queue_runtime_settings(Some("first".into()), None, None);
    save(&mut session, &peer);
    session.queue_runtime_settings(Some("second".into()), None, None);
    save(&mut session, &peer);
    assert_eq!(peer.calls.lock().expect("one pending write").len(), 1);
    receive(&mut session, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(
        session.runtime_settings().pending_model().as_deref(),
        Some("second")
    );
    assert!(tick(&mut session, &peer));
    assert_eq!(
        peer.calls.lock().expect("later write")[1].1["model"],
        "second"
    );
    let old = reply(&peer, 1, json!({"result":{}}));
    session.adopt_native_thread("forked".into());
    session.set_runtime_settings(Some("new-thread".into()), None, None);
    receive(&mut session, old);
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("new-thread")
    );
    assert!(!session.codex_settings().pending());
}

#[test]
fn unsupported_future_updates_keep_explicit_next_turn_overrides_without_global_config_writes() {
    let mut session = session();
    let peer = TestBackend::default();
    session.queue_runtime_settings(Some("next-model".into()), Some("low".into()), None);
    save(&mut session, &peer);
    let settled = receive(
        &mut session,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Unknown method"}}),
        ),
    );
    assert_eq!(
        session.next_turn_runtime_settings().0.as_deref(),
        Some("next-model")
    );
    assert!(session.codex_settings().legacy_overrides());
    assert!(notice(&settled, "direct next-turn overrides"));
    assert_eq!(peer.calls.lock().expect("native write").len(), 1);
    assert_eq!(
        peer.calls.lock().expect("native method")[0].0,
        "thread/settings/update"
    );
}

#[test]
fn models_and_efforts_outside_the_advertised_catalog_are_not_sent() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_available_models(vec![ModelInfo {
        model: "native-model".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]);
    for (model, effort) in [("unadvertised", "low"), ("native-model", "ultra")] {
        session.queue_runtime_settings(Some(model.into()), Some(effort.into()), None);
        save(&mut session, &peer);
        assert!(peer.calls.lock().expect("no invalid writes").is_empty());
    }
}

#[test]
fn local_editor_rejection_preserves_valid_pending_choices_before_apply_or_persist() {
    let mut session = session();
    let peer = TestBackend::default();
    session.set_runtime_settings(Some("native-model".into()), Some("low".into()), None);
    session.set_available_models(vec![ModelInfo {
        model: "native-model".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]);
    let desired = json!({"model":"native-model","effort":"rejected-effort"});
    let mut update = settings::SettingsUpdate::default();
    let scope = session.codex_scope();
    settings::save_values(
        &peer,
        &mut session,
        scope,
        &mut update,
        desired.clone(),
        true,
    );
    assert!(!update.accepted);
    assert_eq!(
        session.next_turn_runtime_settings().1.as_deref(),
        Some("low")
    );
    assert!(peer
        .calls
        .lock()
        .expect("no invalid native write")
        .is_empty());
    assert!(update.effects.iter().any(|effect| matches!(effect,
        settings::SettingsEffect::Rejected { desired: attempted, .. } if attempted == &desired)));
    assert!(update
        .effects
        .iter()
        .all(|effect| !matches!(effect, settings::SettingsEffect::Persist(_))));
    assert!(
        super::settings_contract_support::send(&mut session, "after local rejection")
            .get("effort")
            .is_none()
    );
}
