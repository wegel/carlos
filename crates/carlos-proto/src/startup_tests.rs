//! Keep old create callers valid while rejecting arbitrary backend summary spellings.

use serde_json::json;

use crate::commands::{Command, Request};
use crate::startup::{NativeLaunchOptions, ReasoningSummary};

#[test]
fn old_create_envelopes_keep_an_empty_launch_group_out_of_the_wire() {
    let frame = json!({"jsonrpc":"2.0","id":1,"method":"session/create","params":{
        "operation":{"client":"a".repeat(32),"id":"1"},"backend":"claude","cwd":"/project","name":null}});
    let request: Request = serde_json::from_value(frame.clone()).expect("old caller");
    assert!(matches!(&request.command, Command::Create { launch, .. } if launch.is_empty()));
    assert_eq!(serde_json::to_value(request).expect("create wire"), frame);
}

#[test]
fn exact_child_values_include_empty_without_accepting_arbitrary_summary_parameters() {
    for value in ["", " \u{2003}é🦊 "] {
        let parsed: NativeLaunchOptions =
            serde_json::from_value(json!({"claude_context_window":value}))
                .expect("raw Unicode child value");
        assert_eq!(parsed.claude_context_window.as_deref(), Some(value));
    }
    let parsed: NativeLaunchOptions =
        serde_json::from_value(json!({"reasoning_summary":"none"})).expect("known summary");
    assert_eq!(parsed.reasoning_summary, Some(ReasoningSummary::None));
    assert!(serde_json::from_value::<NativeLaunchOptions>(
        json!({"reasoning_summary":"native/method"})
    )
    .is_err());
}
