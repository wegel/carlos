//! Prove local notices and consent keys leave every session field unchanged.

use crate::session::approval_state::ApprovalQueue;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

use super::state::{AppState, ApprovalChoice, ApprovalRequestKind};
use super::view_models::TerminalSize;
use crate::backend::{BackendClient, BackendKind, EventReceiver};

struct NoBackend;

impl BackendClient for NoBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> anyhow::Result<String> {
        panic!("no backend call expected")
    }
    fn respond(&self, _: &Value, _: Value) -> anyhow::Result<()> {
        panic!("first confirmation must not answer backend")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        panic!("no reply expected")
    }
    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused")
    }
    fn stop(&mut self) {}
}

#[test]
fn review_ownership_ctrl_l_must_preserve_session_status() {
    let mut app = AppState::new("thread".into());
    app.set_status("turn completed");
    let before = format!("{:?}", app.session);
    super::input_events::handle_terminal_event(
        &NoBackend,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
        TerminalSize {
            width: 80,
            height: 24,
        },
    );
    assert_eq!(
        format!("{:?}", app.session),
        before,
        "local key must preserve every session field"
    );
    assert_eq!(
        app.session.status(),
        "turn completed",
        "a local selection-clear hint belongs to ViewState"
    );
}

#[test]
fn review_ownership_first_allow_must_preserve_request_facts() {
    let mut app = AppState::new("thread".into());
    app.claude.enabled = true;
    let details = vec!["native request details".to_string()];
    app.set_pending_approval(crate::session::approval_state::ApprovalRequestDraft {
        request_id: json!("request"),
        method: "claude/native".into(),
        kind: ApprovalRequestKind::ClaudeNative,
        title: "Approve".into(),
        detail_lines: details.clone(),
        requested_permissions: Some(json!({"defaultToNo":true,"toolName":"Bash"})),
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: true,
    });
    let before = format!("{:?}", app.session);
    super::turn_approval::respond_to_pending_approval(&NoBackend, &mut app, ApprovalChoice::Accept);
    assert_eq!(
        format!("{:?}", app.session),
        before,
        "first consent must preserve every session field"
    );
    assert_eq!(
        app.view.confirm_allow,
        app.session
            .approval()
            .pending()
            .expect("pending")
            .entry_id()
    );
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("pending")
            .facts()
            .detail_lines,
        details,
        "the local press-y-again hint belongs to view confirmation state"
    );
}

#[test]
fn every_local_hint_and_first_consent_leave_all_session_fields_unchanged() {
    use super::view_hint_support::{key, request, Client};
    let kinds = [
        ApprovalRequestKind::CommandExecution,
        ApprovalRequestKind::FileChange,
        ApprovalRequestKind::Permissions,
        ApprovalRequestKind::LegacyExecCommand,
        ApprovalRequestKind::LegacyApplyPatch,
        ApprovalRequestKind::ClaudeExitPlanMode,
        ApprovalRequestKind::ClaudeNative,
        ApprovalRequestKind::ClaudeQuestion,
        ApprovalRequestKind::CodexQuestion,
        ApprovalRequestKind::CodexForm,
    ];
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for kind in kinds {
            let client = Client::new(backend);
            let mut app = AppState::new("thread".into());
            app.claude.enabled = backend == BackendKind::Claude;
            app.set_status("turn completed");
            app.set_pending_approval(request(kind, "request"));
            let before = format!("{:?}", app.session);
            super::turn_approval::respond_to_pending_approval(
                &client,
                &mut app,
                ApprovalChoice::Accept,
            );
            if client.calls.lock().expect("calls").is_empty() {
                assert_eq!(format!("{:?}", app.session), before, "{backend:?} {kind:?}");
            }
            let before = format!("{:?}", app.session);
            app.set_hint("editor field error");
            app.toggle_dictation_endpoint_mode();
            app.set_hint("");
            key(&client, &mut app, KeyCode::Char('x'), KeyModifiers::NONE);
            assert_eq!(format!("{:?}", app.session), before);
        }
    }
}

#[test]
fn native_answer_editors_keep_session_facts_until_they_send_an_answer() {
    use super::view_hint_support::Client;
    for (kind, payload) in [
        ("permission", json!({"toolName":"Bash","defaultToNo":true})),
        (
            "permission",
            json!({"toolName":"AskUserQuestion","defaultToNo":true,
            "input":{"questions":[{"question":"Format?","header":"Format",
                "options":[{"label":"Summary"}]}]}}),
        ),
        (
            "elicitation",
            json!({"serverName":"counter","defaultToNo":true,
            "requestedSchema":{"type":"object","properties":{"age":{"type":"integer"}},
                "required":["age"]}}),
        ),
    ] {
        let client = Client::new(BackendKind::Claude);
        let mut app = AppState::new("thread".into());
        app.claude.enabled = true;
        app.set_status("turn completed");
        super::claude_callbacks::receive_callback(
            &mut app,
            &json!({"callbackId":"request","kind":kind,"payload":payload}),
        );
        let before = format!("{:?}", app.session);
        super::turn_approval::respond_to_pending_approval(
            &client,
            &mut app,
            ApprovalChoice::Accept,
        );
        assert!(client.calls.lock().expect("calls").is_empty());
        assert_eq!(format!("{:?}", app.session), before);
    }
}

#[test]
fn duplicate_adoption_keeps_initial_guidance_and_second_accept_but_clears_extra_lines() {
    use super::approval_lifetime_support::{accept, app, callback, fill, Client};
    for through_answer_target in [false, true] {
        for fields in [false, true] {
            let client = Client::new(crate::backend::BackendKind::Claude);
            let mut app = app(crate::backend::BackendKind::Claude);
            let payload = if fields {
                json!({"toolName":"AskUserQuestion","defaultToNo":true,"input":{"questions":[{
                    "question":"Color?","header":"Color","options":[{"label":"Blue"}]}]}})
            } else {
                json!({"toolName":"Bash","defaultToNo":true})
            };
            callback(&client, &mut app, "one", "permission", payload);
            fill(&client, &mut app);
            let original = app.session.approval().pending().expect("original callback");
            let original_id = original.entry_id();
            let original = original.to_draft();
            let initial = card_capture(&app);
            assert!(initial.to_string().contains("twice"));
            accept(&client, &mut app);
            assert!(client.answers().is_empty());
            app.prepend_approval_hint("extra local hint");
            if !through_answer_target {
                app.set_pending_approval(original.clone());
            } else {
                let copy = app
                    .session
                    .approval_answer(original_id.expect("card id"))
                    .expect("active answer target");
                app.set_pending_approval(copy);
            }
            assert_eq!(
                app.session
                    .approval()
                    .pending()
                    .expect("duplicate")
                    .entry_id(),
                original_id
            );
            assert_eq!(
                card_capture(&app),
                initial,
                "fields={fields} through_answer_target={through_answer_target}"
            );
            accept(&client, &mut app);
            assert_eq!(
                client.answers().len(),
                1,
                "duplicate keeps legitimate second accept"
            );
            app.assert_entry_invariants();
        }
    }
}

fn card_capture(app: &AppState) -> Value {
    use super::view_models::TerminalSize;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    let screens: Vec<_> = [(160, 48), (60, 20)]
        .into_iter()
        .map(|(width, height)| {
            let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
            super::overlay_render::draw_approval_overlay(
                &mut buffer,
                TerminalSize {
                    width: width as usize,
                    height: height as usize,
                },
                app,
            );
            buffer
                .content
                .iter()
                .map(|cell| format!("{}:{:?}", cell.symbol(), cell.style()))
                .collect::<Vec<_>>()
        })
        .collect();
    let pending = app.session.approval().pending().expect("card");
    json!({"details":app.approval_detail_lines(pending).collect::<Vec<_>>(),"screens":screens})
}

#[test]
fn queued_and_retired_copies_cannot_reuse_an_armed_card_id() {
    use super::approval_lifetime_support::{accept, app, callback, Client};
    for queued in [false, true] {
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        callback(
            &client,
            &mut app,
            "one",
            "permission",
            json!({"toolName":"Bash","defaultToNo":true}),
        );
        let original = if queued {
            callback(
                &client,
                &mut app,
                "two",
                "permission",
                json!({"toolName":"Bash","defaultToNo":true}),
            );
            &app.session.approval().queued(ApprovalQueue::Claude)[0]
        } else {
            app.session.approval().pending().expect("active")
        };
        let original_id = original.entry_id();
        let original = original.to_draft();
        app.view.confirm_allow = original_id;
        if !queued {
            app.clear_pending_approval();
        }
        app.set_pending_approval(original.clone());
        assert_ne!(
            app.session
                .approval()
                .pending()
                .expect("adopted")
                .entry_id(),
            original_id
        );
        accept(&client, &mut app);
        assert!(
            client.answers().is_empty(),
            "copied id needs its own first accept"
        );
        app.assert_entry_invariants();
    }
}

#[test]
fn retired_card_cannot_return_through_queue_focus_or_answer_a_reused_native_id() {
    use super::approval_lifetime_support::{accept, app, callback, Client};
    use crate::session::approval_state::ApprovalQueue;
    for replacement in [false, true] {
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        callback(
            &client,
            &mut app,
            "one",
            "permission",
            json!({
            "toolName":"Bash","input":{"command":"echo original"},"defaultToNo":true}),
        );
        accept(&client, &mut app);
        assert!(client.answers().is_empty());
        let retired = app
            .session
            .approval()
            .pending()
            .expect("original")
            .entry_id()
            .expect("issued id");
        app.clear_pending_approval();
        assert!(app.session.approval().pending().is_none());
        if replacement {
            callback(
                &client,
                &mut app,
                "one",
                "permission",
                json!({
                "toolName":"Bash","input":{"command":"echo different"},"defaultToNo":true}),
            );
        }
        let current = app.session.approval().pending().and_then(|p| p.entry_id());
        assert_ne!(current, Some(retired));
        for queue in [ApprovalQueue::Claude, ApprovalQueue::Codex] {
            app.session.focus_approval(retired, queue);
            assert_eq!(
                app.session.approval().pending().and_then(|p| p.entry_id()),
                current
            );
        }
        assert!(app.session.approval_answer(retired).is_none());
        app.retain_live_approval_hints();
        accept(&client, &mut app);
        assert!(
            client.answers().is_empty(),
            "queue focus must not restore retired consent"
        );
        accept(&client, &mut app);
        assert_eq!(client.answers().len(), usize::from(replacement));
        app.assert_entry_invariants();
    }
}
