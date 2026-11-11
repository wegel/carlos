//! Check independent live/future outcomes and external settings changes without synthetic input.

use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};
use crate::app::codex_settings::{label, save, started, tick};
use crate::app::input_events::handle_terminal_event;
use crate::app::turn_submit::submit_turn_text;
use crate::app::TerminalSize;
use crate::protocol_params::ModelInfo;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn reply(peer: &Peer, index: usize, response: Value) -> Value {
    let (method, params) = peer.controls.borrow()[index].clone();
    json!({"method":"carlos/codex/controlReply","params":{"requestMethod":method,
        "requestParams":params,"requestContext":peer.contexts.borrow()[index],"response":response}})
}

#[test]
fn future_success_and_live_failure_are_separate_and_do_not_relabel_the_running_request() {
    let mut app = app();
    let peer = Peer::default();
    app.set_runtime_settings(
        Some("old".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    app.active_turn_id = Some("active".into());
    started(&mut app, "active");
    app.queue_runtime_settings(Some("new".into()), Some("low".into()), Some("none".into()));
    save(&peer, &mut app);
    assert_eq!(
        peer.controls
            .borrow()
            .iter()
            .map(|entry| entry.0.as_str())
            .collect::<Vec<_>>(),
        ["thread/settings/update", "turn/settings/update"]
    );
    assert_eq!(peer.controls.borrow()[1].1["turnId"], "active");
    receive(
        &mut app,
        reply(
            &peer,
            1,
            json!({"error":{"code":-32601,"message":"method not found"}}),
        ),
    );
    assert!(app.codex.settings.pending.is_some());
    receive(&mut app, reply(&peer, 0, json!({"result":{}})));
    assert!(app.codex.settings.pending.is_none());
    assert_eq!(app.runtime.current_model.as_deref(), Some("new"));
    assert!(label(&app).starts_with("old/high/concise → future new/low/none"));
    assert!(app
        .messages
        .last()
        .expect("visible report")
        .text
        .contains("active turn unchanged"));
    assert!(app.runtime.pending_model.is_none());
}

#[test]
fn a_newer_external_native_setting_wins_over_an_older_empty_acknowledgement() {
    let mut app = app();
    let peer = Peer::default();
    app.queue_runtime_settings(
        Some("requested".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    save(&peer, &mut app);
    receive(
        &mut app,
        json!({"method":"thread/settings/updated","params":{"threadId":"root",
        "threadSettings":{"model":"external-newer","effort":"low","summary":null,"unknown":"kept"}}}),
    );
    receive(&mut app, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(
        app.next_turn_runtime_settings(),
        (Some("external-newer".into()), Some("low".into()), None)
    );
    assert_eq!(app.codex.settings.future["unknown"], "kept");
}

#[test]
fn later_local_choices_wait_without_losing_their_values_and_old_epoch_replies_are_ignored() {
    let mut app = app();
    let peer = Peer::default();
    app.queue_runtime_settings(Some("first".into()), None, None);
    save(&peer, &mut app);
    app.queue_runtime_settings(Some("second".into()), None, None);
    save(&peer, &mut app);
    assert_eq!(peer.controls.borrow().len(), 1);
    receive(&mut app, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(app.runtime.pending_model.as_deref(), Some("second"));
    assert!(tick(&peer, &mut app));
    assert_eq!(peer.controls.borrow()[1].1["model"], "second");
    let old = reply(&peer, 1, json!({"result":{}}));
    app.set_thread_id("forked");
    app.set_runtime_settings(Some("new-thread".into()), None, None);
    receive(&mut app, old);
    assert_eq!(app.runtime.current_model.as_deref(), Some("new-thread"));
    assert!(app.codex.settings.pending.is_none());
}

#[test]
fn unsupported_future_updates_keep_explicit_next_turn_overrides_without_global_config_writes() {
    let mut app = app();
    let peer = Peer::default();
    app.queue_runtime_settings(Some("next-model".into()), Some("low".into()), None);
    save(&peer, &mut app);
    receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Unknown method"}}),
        ),
    );
    assert_eq!(
        app.next_turn_runtime_settings().0.as_deref(),
        Some("next-model")
    );
    assert!(app.codex.settings.legacy_overrides);
    assert!(app
        .messages
        .last()
        .expect("fallback visible")
        .text
        .contains("direct next-turn overrides"));
    assert_eq!(peer.controls.borrow().len(), 1);
    assert_eq!(peer.controls.borrow()[0].0, "thread/settings/update");
}

#[test]
fn models_and_efforts_outside_the_advertised_catalog_are_not_sent() {
    let mut app = app();
    let peer = Peer::default();
    app.set_available_models(vec![ModelInfo {
        model: "native-model".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]);
    for (model, effort) in [("unadvertised", "low"), ("native-model", "ultra")] {
        app.queue_runtime_settings(Some(model.into()), Some(effort.into()), None);
        save(&peer, &mut app);
        assert!(peer.controls.borrow().is_empty());
    }
}

#[test]
fn ordinary_prompts_inherit_native_choices_during_pending_apply_and_external_changes() {
    let mut app = app();
    let peer = Peer::default();
    peer.allow_turns.set(true);
    app.set_runtime_settings(Some("original".into()), Some("low".into()), None);
    app.queue_runtime_settings(Some("requested".into()), Some("high".into()), None);
    save(&peer, &mut app);
    submit_turn_text(&peer, &mut app, "while pending".into());
    assert_eq!(app.runtime.current_model.as_deref(), Some("original"));
    assert_eq!(app.runtime.pending_model.as_deref(), Some("requested"));
    receive(
        &mut app,
        json!({"method":"thread/settings/updated","params":{"threadId":"root",
        "threadSettings":{"model":"external","effort":"low"}}}),
    );
    submit_turn_text(&peer, &mut app, "external native defaults".into());
    receive(&mut app, reply(&peer, 0, json!({"result":{}})));
    assert_eq!(app.runtime.current_model.as_deref(), Some("external"));
    for (_, params) in peer.calls.borrow().iter() {
        for key in ["model", "effort", "summary"] {
            assert!(params.get(key).is_none());
        }
    }
}

#[test]
fn native_validation_rejection_keeps_error_data_and_editor_draft_out_of_subsequent_prompts() {
    let mut app = app();
    let peer = Peer::default();
    peer.allow_turns.set(true);
    app.set_runtime_settings(Some("last-valid".into()), Some("low".into()), None);
    app.queue_runtime_settings(Some("rejected".into()), Some("high".into()), None);
    save(&peer, &mut app);
    receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32600,"message":"Managed model policy rejected this choice","data":{"policy":"managed"}}}),
        ),
    );
    assert_eq!(app.runtime.current_model.as_deref(), Some("last-valid"));
    assert!(app.runtime.pending_model.is_none());
    assert!(!app.codex.settings.legacy_overrides);
    assert_eq!(app.codex.settings.errors["future"]["code"], -32600);
    assert_eq!(
        app.codex.settings.errors["future"]["data"]["policy"],
        "managed"
    );
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "rejected");
    assert_eq!(app.model_settings_effort_value(), "high");
    app.close_model_settings();
    submit_turn_text(&peer, &mut app, "use valid native settings".into());
    assert!(peer.calls.borrow()[0].1.get("model").is_none());
}

#[test]
fn local_editor_rejection_preserves_valid_pending_choices_before_apply_or_persist() {
    let mut app = app();
    let peer = Peer::default();
    peer.allow_turns.set(true);
    app.set_runtime_settings(Some("native-model".into()), Some("low".into()), None);
    app.set_available_models(vec![ModelInfo {
        model: "native-model".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]);
    app.open_model_settings();
    app.runtime
        .model_settings_effort_options
        .push("rejected-effort".into());
    app.runtime.model_settings_effort_index = 1;
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        TerminalSize {
            width: 100,
            height: 32,
        },
    );
    assert!(app.runtime.show_model_settings);
    assert_eq!(app.model_settings_effort_value(), "rejected-effort");
    assert_eq!(app.next_turn_runtime_settings().1.as_deref(), Some("low"));
    assert!(peer.controls.borrow().is_empty());
    assert!(app.codex.settings.editor_error.is_some());
    app.close_model_settings();
    submit_turn_text(&peer, &mut app, "after local rejection".into());
    assert!(peer.calls.borrow()[0].1.get("effort").is_none());
}

#[test]
fn only_definitively_unsupported_native_settings_allow_direct_prompt_overrides() {
    let mut app = app();
    let peer = Peer::default();
    peer.allow_turns.set(true);
    app.queue_runtime_settings(Some("explicit-compat".into()), Some("low".into()), None);
    save(&peer, &mut app);
    receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Method not found"}}),
        ),
    );
    submit_turn_text(&peer, &mut app, "older native".into());
    assert_eq!(peer.calls.borrow()[0].1["model"], "explicit-compat");
    assert_eq!(peer.calls.borrow()[0].1["effort"], "low");
}

#[test]
fn a_native_model_without_effort_choices_never_gets_an_invented_effort_override() {
    let mut app = app();
    let peer = Peer::default();
    peer.allow_turns.set(true);
    app.set_available_models(vec![ModelInfo {
        model: "native-no-effort".into(),
        display_name: "Native".into(),
        supported_efforts: Vec::new(),
        default_effort: None,
        is_default: true,
    }]);
    app.set_runtime_settings(
        Some("native-no-effort".into()),
        Some("old-effort".into()),
        None,
    );
    app.open_model_settings();
    assert!(!app
        .runtime
        .model_settings_fields()
        .contains(&crate::app::state::ModelSettingsField::Effort));
    save(&peer, &mut app);
    assert!(peer.controls.borrow()[0].1.get("effort").is_none());
    receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32601,"message":"Method not found"}}),
        ),
    );
    submit_turn_text(&peer, &mut app, "no forced effort".into());
    assert!(peer.calls.borrow()[0].1.get("effort").is_none());
}
