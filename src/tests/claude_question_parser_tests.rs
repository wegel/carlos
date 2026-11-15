//! Preserve tolerant MCP choice metadata in the answer editor.

use carlos_core::session::claude::callback_questions::ClaudeQuestion;
use serde_json::json;

use crate::app::claude_question_state::NativeDraft;

#[test]
fn incomplete_choice_metadata_remains_accepted_in_mcp_forms() {
    for rule in [
        json!({"type":"string","enum":[17],"enumNames":["Seventeen"],"default":17}),
        json!({"type":"string","oneOf":[{"const":17}],"default":17}),
        json!({"type":"string","anyOf":[{"const":17}],"default":17}),
    ] {
        let payload = json!({"requestedSchema":{"properties":{"value":rule}}});
        let draft =
            NativeDraft::from_description(ClaudeQuestion::from_payload("elicitation", &payload));
        assert_eq!(
            draft.fields[0]
                .mcp_value()
                .expect("existing tolerant choice"),
            Some(json!(17))
        );
    }
}
