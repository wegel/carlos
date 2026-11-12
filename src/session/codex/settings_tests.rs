//! Verify native settings scopes and one-time save effects without a terminal.

use serde_json::json;

use super::scope::CodexScope;
use super::settings::{observed, save_values, SettingsEffect, SettingsUpdate};
use super::settings_reply::reply;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn accepted_future_settings_offer_one_save_before_the_final_notice() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let peer = TestBackend::default();
    let scope = CodexScope {
        enabled: true,
        closed: false,
        epoch: 9,
    };
    let desired = json!({"model":"chosen","effort":"high","summary":"concise"});
    let mut update = SettingsUpdate::default();
    save_values(
        &peer,
        &mut session,
        scope,
        &mut update,
        desired.clone(),
        true,
    );
    assert!(update.accepted);
    assert!(session.codex_settings().pending());
    assert!(matches!(update.effects[0], SettingsEffect::ClearRejected));
    assert!(matches!(update.effects[1], SettingsEffect::Notice(_)));
    let call = peer.calls.lock().expect("calls")[0].clone();
    assert_eq!(call.0, "thread/settings/update");
    assert_eq!(
        call.1,
        json!({"threadId":"root","model":"chosen","effort":"high","summary":"concise"})
    );
    let frame = json!({"requestMethod":call.0,"requestParams":call.1,
        "requestContext":call.2,"response":{"result":{}}});
    let mut settled = SettingsUpdate::default();
    reply(&mut session, scope, &mut settled, &frame);
    assert!(!session.codex_settings().pending());
    assert_eq!(session.codex_settings().future(), &desired);
    assert!(matches!(&settled.effects[0], SettingsEffect::Persist(value) if value == &desired));
    assert!(matches!(settled.effects[1], SettingsEffect::Notice(_)));
    let count = settled.effects.len();
    reply(&mut session, scope, &mut settled, &frame);
    assert_eq!(
        settled.effects.len(),
        count,
        "a duplicate acknowledgment cannot save again"
    );
}

#[test]
fn newer_native_settings_defeat_an_older_acknowledgment_without_an_editor() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let peer = TestBackend::default();
    let scope = CodexScope {
        enabled: true,
        closed: false,
        epoch: 2,
    };
    let mut update = SettingsUpdate::default();
    save_values(
        &peer,
        &mut session,
        scope,
        &mut update,
        json!({"model":"requested"}),
        false,
    );
    let call = peer.calls.lock().expect("calls")[0].clone();
    observed(
        &mut session,
        &json!({"threadId":"root","threadSettings":{"model":"external","unknown":"retained"}}),
    );
    reply(
        &mut session,
        scope,
        &mut update,
        &json!({"requestMethod":call.0,
        "requestParams":call.1,"requestContext":call.2,"response":{"result":{}}}),
    );
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("external")
    );
    assert_eq!(session.codex_settings().future()["unknown"], "retained");
    assert!(!session.codex_settings().pending());
}
