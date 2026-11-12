//! Guard consent by card identity and keep hint records only for live requests.

use crossterm::event::{KeyCode, KeyModifiers};
use serde_json::json;

use super::approval_lifetime_support::{accept, app, callback, key, payloads, server, Client};
use super::state::ApprovalRequestKind;
use crate::backend::BackendKind;
use crate::session::approval_state::ApprovalQueue;

#[test]
fn a_new_helper_cannot_inherit_consent_for_a_reused_native_id() {
    let client = Client::new(BackendKind::Claude);
    let mut app = app(BackendKind::Claude);
    callback(
        &client,
        &mut app,
        "reused-id",
        "permission",
        json!({"toolName":"Bash","input":{"command":"echo first"},"defaultToNo":true}),
    );
    accept(&client, &mut app);
    assert!(client.answers().is_empty());
    let first = app.session.approval().pending().expect("first").entry_id();
    server(
        &client,
        &mut app,
        json!({"method":"claude/sessionChanged",
        "params":{"generation":2,"sessionId":"second-thread"}}),
    );
    assert!(app.view.confirm_allow.is_none());
    callback(
        &client,
        &mut app,
        "reused-id",
        "permission",
        json!({"toolName":"Bash","input":{"command":"echo different"},"defaultToNo":true}),
    );
    assert_ne!(
        app.session.approval().pending().expect("second").entry_id(),
        first
    );
    accept(&client, &mut app);
    assert!(
        client.answers().is_empty(),
        "new request needs its own first consent key"
    );
    accept(&client, &mut app);
    assert_eq!(
        client.answers(),
        [json!(["answer","reused-id",{"behavior":"allow"}])]
    );
}

#[test]
fn replacing_a_card_with_the_same_native_id_requires_fresh_consent() {
    for queued_draft in [false, true] {
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        callback(
            &client,
            &mut app,
            "one",
            "permission",
            json!({"toolName":"Bash","defaultToNo":true}),
        );
        accept(&client, &mut app);
        let old = app.session.approval().pending().expect("first").entry_id();
        let mut replacement = app.session.approval().pending().expect("first").to_draft();
        replacement.requested_permissions = Some(json!({"toolName":"Bash",
            "defaultToNo":true,"input":{"command":"different command"}}));
        if queued_draft {
            let queued = app
                .session
                .enqueue_approval(replacement.clone(), ApprovalQueue::Claude);
            // Only facts can be copied out of a queued card, never its identity.
            replacement = app
                .session
                .approval()
                .queued(ApprovalQueue::Claude)
                .iter()
                .find(|p| p.entry_id() == Some(queued))
                .expect("queued")
                .to_draft();
        }
        app.set_pending_approval(replacement);
        assert_ne!(
            app.session
                .approval()
                .pending()
                .expect("replacement")
                .entry_id(),
            old
        );
        accept(&client, &mut app);
        assert!(client.answers().is_empty());
        accept(&client, &mut app);
        assert_eq!(client.answers().len(), 1);
        app.assert_entry_invariants();
    }
}

#[test]
fn helper_switch_and_failure_drop_all_discarded_queued_hints() {
    for fail in [false, true] {
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        for id in ["one", "two", "three"] {
            callback(
                &client,
                &mut app,
                id,
                "permission",
                json!({"toolName":"Bash","defaultToNo":true}),
            );
        }
        for _ in 0..3 {
            accept(&client, &mut app);
            key(&client, &mut app, KeyCode::F(4), KeyModifiers::NONE);
        }
        assert!(client.answers().is_empty());
        assert_eq!(app.approval_hint_count(), 3);
        if fail {
            server(
                &client,
                &mut app,
                json!({"method":"claude/closed",
                "params":{"generation":1,"reason":"failure"}}),
            );
            assert_eq!(app.approval_hint_count(), 0);
        }
        server(
            &client,
            &mut app,
            json!({"method":"claude/sessionChanged",
            "params":{"generation":2,"sessionId":"second-thread"}}),
        );
        assert_eq!(app.approval_hint_count(), usize::from(!fail));
        app.assert_entry_invariants();
    }
}

#[test]
fn native_reset_preserves_process_callbacks_consent_and_every_live_hint() {
    let client = Client::new(BackendKind::Claude);
    let mut app = app(BackendKind::Claude);
    for id in ["one", "two", "three"] {
        callback(
            &client,
            &mut app,
            id,
            "permission",
            json!({"toolName":"Bash","defaultToNo":true}),
        );
    }
    // Arm each card once without sending an answer.
    for _ in 0..3 {
        key(&client, &mut app, KeyCode::F(4), KeyModifiers::NONE);
        accept(&client, &mut app);
    }
    let consent = app.view.confirm_allow;
    let cards: Vec<_> = app.live_approval_cards().map(|p| p.entry_id()).collect();
    let hints = app.approval_hint_count();
    server(
        &client,
        &mut app,
        json!({"method":"claude/conversationReset",
        "params":{"generation":1,"new_conversation_id":"reset"}}),
    );
    assert_eq!(
        app.live_approval_cards()
            .map(|p| p.entry_id())
            .collect::<Vec<_>>(),
        cards
    );
    assert_eq!(app.view.confirm_allow, consent);
    assert_eq!(app.approval_hint_count(), hints);
    let before = client.answers().len();
    accept(&client, &mut app);
    assert_eq!(client.answers().len(), before + 1);
}

#[test]
fn callback_facts_never_store_carlos_keyboard_or_browser_instructions() {
    for (kind, mut payload) in payloads() {
        payload["defaultToNo"] = json!(true);
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        callback(&client, &mut app, "one", kind, payload);
        let pending = app.session.approval().pending().expect("callback");
        for line in &pending.facts().detail_lines {
            assert!(
                !["Press y", "Ctrl+S", "Ctrl+Y", "Accept consents"]
                    .iter()
                    .any(|key| line.contains(key)),
                "{kind}: {line}"
            );
        }
        let details = app.approval_detail_lines(pending).collect::<Vec<_>>();
        assert!(details.iter().any(|line| line.contains("twice")));
        if pending.facts().kind == ApprovalRequestKind::ClaudeQuestion
            && pending
                .facts()
                .requested_permissions
                .as_ref()
                .is_some_and(|p| p["url"].is_string())
        {
            assert!(details[2].starts_with("Ctrl+Y"));
            assert!(details
                .last()
                .expect("browser consent")
                .starts_with("Accept consents"));
        }
    }
}

#[test]
fn codex_question_facts_keep_policy_without_local_key_names() {
    use super::approval_lifetime_support::METHODS;
    for (method, params) in [
        (
            METHODS[5],
            json!({"threadId":"thread","turnId":"turn","itemId":"item",
            "isBlocking":false,"questions":[{"id":"q","header":"Header","question":"Color?","options":[]}]}),
        ),
        (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"url",
            "url":"https://example.com/path"}),
        ),
    ] {
        let client = Client::new(BackendKind::Codex);
        let mut app = app(BackendKind::Codex);
        server(
            &client,
            &mut app,
            json!({"method":method,"id":"one","params":params}),
        );
        let pending = app.session.approval().pending().expect("question");
        assert!(pending
            .facts()
            .detail_lines
            .iter()
            .all(|line| !line.contains("Ctrl+Y") && !line.contains("Esc")));
        let visible = app
            .approval_detail_lines(pending)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(visible.contains(if method == METHODS[5] {
            "Esc"
        } else {
            "Ctrl+Y"
        }));
    }
}

#[test]
fn the_shared_invariant_checker_rejects_an_orphaned_card_hint() {
    let mut app = app(BackendKind::Claude);
    let orphan = app.session.transcript_allocate_card();
    app.view.approval_hints.register_callback(orphan, false);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.assert_entry_invariants();
    }))
    .is_err());
}

#[test]
fn codex_retirement_removes_only_discarded_card_hints() {
    use super::approval_lifetime_support::{request, METHODS};
    for tail in ["active", "queued", "fork", "closed", "reconnect", "history"] {
        let client = Client::new(BackendKind::Codex);
        let mut app = app(BackendKind::Codex);
        for id in ["one", "two", "three"] {
            request(&client, &mut app, METHODS[0], id);
        }
        for _ in 0..3 {
            app.prepend_approval_hint("local card hint");
            key(&client, &mut app, KeyCode::F(4), KeyModifiers::NONE);
        }
        assert_eq!(app.approval_hint_count(), 3);
        match tail {
            "active" | "queued" => server(
                &client,
                &mut app,
                json!({"method":"serverRequest/resolved",
                "params":{"threadId":"thread","requestId":if tail == "active" { "one" } else { "two" }}}),
            ),
            "fork" => app.reset_for_forked_thread("fork"),
            "closed" | "reconnect" => {
                server(
                    &client,
                    &mut app,
                    json!({"method":"carlos/codex/closed","params":{"message":"failure"}}),
                );
                if tail == "reconnect" {
                    key(&client, &mut app, KeyCode::F(5), KeyModifiers::NONE);
                }
            }
            "history" => super::item_history::replace_history_from_thread(
                &mut app,
                &json!({"id":"thread","turns":[]}),
            ),
            _ => unreachable!(),
        }
        let expected = match tail {
            "active" | "queued" => 2,
            "history" => 3,
            _ => 0,
        };
        assert_eq!(app.approval_hint_count(), expected, "{tail}");
        app.assert_entry_invariants();
    }
}

#[test]
fn the_startup_picker_cannot_carry_an_old_apps_consent_or_hints() {
    // Every backend creates AppState after its picker returns; no live-app picker exists.
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let client = Client::new(backend);
        let mut old = app(backend);
        let mut pending =
            super::view_hint_support::request(ApprovalRequestKind::ClaudeNative, "one");
        pending.requested_permissions = None;
        old.set_pending_approval(pending);
        old.prepend_approval_hint("old card hint");
        let consent = old
            .live_approval_cards()
            .next()
            .expect("old card")
            .entry_id();
        old.view.confirm_allow = consent;
        drop(old);
        let fresh = app(backend);
        assert!(fresh.session.approval().pending().is_none());
        assert!(fresh.view.confirm_allow.is_none());
        assert_eq!(fresh.approval_hint_count(), 0);
        fresh.assert_entry_invariants();
        assert!(client.answers().is_empty());
    }
}
