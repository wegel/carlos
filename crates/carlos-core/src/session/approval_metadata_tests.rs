//! Bound caller display labels and refresh only the original browser card's readonly details.

use serde_json::json;

use super::events::Change;
use super::state::SessionState;

#[test]
fn long_unicode_callers_clip_with_an_ellipsis_without_changing_the_original_request() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    let label = "日本語🌍".repeat(300);
    state.receive_claude_callback(&json!({"callbackId":"long-caller","kind":"permission",
        "payload":{"toolName":label,"agentID":label,"input":{}}}));
    let card = &state.export_approvals()[0];
    let caller = card.caller.as_ref().expect("native caller");
    assert!(caller.len() <= 4096);
    assert!(caller.ends_with('…'));
    assert_eq!(
        state
            .approval()
            .pending()
            .expect("original card")
            .facts()
            .requested_permissions
            .as_ref()
            .expect("original native fields")["toolName"],
        label
    );
}

#[test]
fn early_browser_completion_refreshes_exact_card_details_without_changing_consent_fields() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.receive_claude_callback(
        &json!({"callbackId":"original-browser","kind":"elicitation",
        "payload":{"mode":"url","serverName":"native-server","elicitationId":"original-id",
            "url":"https://native.test/device","urlHost":"native.test"}}),
    );
    let before = state.publish().snapshot(10).approvals.remove(0);
    let completion = json!({"mcp_server_name":"native-server","elicitation_id":"original-id"});
    assert_eq!(
        state.receive_claude_browser_completion(&completion),
        Some(Some("original-browser".into()))
    );
    let published = state.publish();
    assert!(published
        .records()
        .iter()
        .any(|record| matches!(record.change, Change::Approval(_))));
    let after = published.snapshot(10).approvals.remove(0);
    assert_eq!(
        (&after.id, &after.input, &after.choices, &after.caller),
        (&before.id, &before.input, &before.choices, &before.caller)
    );
    assert!(after
        .details
        .iter()
        .any(|line| line.contains("Your consent is still required")));
    assert!(!after.submitted);
    assert_eq!(state.receive_claude_browser_completion(&completion), None);
}
