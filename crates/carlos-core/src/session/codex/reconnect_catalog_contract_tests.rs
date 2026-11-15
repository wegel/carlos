//! Keep fresh native catalog and name facts authoritative across buffered callbacks.

use serde_json::json;

use super::reconnect_contract_support::{ready, receive, session, snapshot};
use crate::session::test_backend::TestBackend;

#[test]
fn fresh_catalog_drops_old_direct_override_fallback_without_republishing_pending_choices() {
    let peer = TestBackend::default();
    let mut state = session();
    let mut first = snapshot("one", "idle");
    first.catalog =
        json!({"data":[{"model":"a","displayName":"A"},{"model":"b","displayName":"B"}]});
    ready(&peer, &mut state, first);
    state.set_codex_settings_legacy_for_test(true);
    state.close_codex();
    state.set_codex_rebuilding_for_test(true);
    let mut second = snapshot("two", "idle");
    second.runtime =
        json!({"result":{"thread":{"id":"root"},"model":"a","reasoningEffort":"low"}}).to_string();
    second.catalog =
        json!({"data":[{"model":"b","displayName":"B"},{"model":"a","displayName":"A"}]});
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"thread/settings/updated","params":{"threadId":"root","threadSettings":{"model":"OLD_BUFFERED"}}}),
    );
    ready(&peer, &mut state, second);
    assert_eq!(
        state.runtime_settings().current_model().as_deref(),
        Some("a")
    );
    assert!(!state.codex_settings().legacy_overrides());
    assert!(state.runtime_settings().pending_model().is_none());
}

#[test]
fn fresh_metadata_clears_old_names_and_a_newer_rename_fences_only_the_delayed_name() {
    let peer = TestBackend::default();
    let mut state = session();
    state.set_thread_name(Some("old".into()));
    let mut fresh = snapshot("one", "idle");
    fresh.thread["name"] = json!(null);
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"thread/name/updated","params":{"threadId":"root","threadName":"BUFFERED_OLD"}}),
    );
    ready(&peer, &mut state, fresh);
    assert!(state.thread_name().is_none());
    super::reconnect_runtime::tick(&peer, &mut state);
    let context = peer.calls.lock().expect("native context")[0].2.clone();
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"thread/name/updated","params":{"threadId":"root","threadName":"newer 日本語"}}),
    );
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestContext":context,"requestParams":{"threadId":"root"},"response":{"result":{"thread":{
            "id":"root","name":"STALE_NAME","model":"fresh-model","reasoningEffort":"low"}}}}}),
    );
    assert_eq!(state.thread_name().as_deref(), Some("newer 日本語"));
    assert_eq!(
        state.runtime_settings().current_model().as_deref(),
        Some("fresh-model")
    );
    assert!(state.codex_connection().runtime_refresh());
}
