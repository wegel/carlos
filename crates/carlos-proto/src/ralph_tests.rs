//! Keep old explicit requests compatible and reject unrelated prompt-option fields.

use serde_json::json;

use super::RalphOptions;
use crate::commands::Command;

#[test]
fn old_explicit_ralph_request_omits_options_in_both_directions() {
    let value = json!({"method":"session/setRalph", "params":{
        "session":"11111111111111111111111111111111",
        "operation":{"client":"22222222222222222222222222222222", "id":"1"},
        "on":true,
    }});
    let command: Command = serde_json::from_value(value.clone()).expect("old request");
    assert!(matches!(
        &command,
        Command::SetRalph {
            options: None,
            on: true,
            ..
        }
    ));
    assert_eq!(serde_json::to_value(command).expect("old output"), value);
}

#[test]
fn empty_options_reset_explicit_choices_without_accepting_backend_methods() {
    let options: RalphOptions = serde_json::from_value(json!({})).expect("default choices");
    assert!(!options.has_overrides());
    assert_eq!(
        serde_json::to_value(&options).expect("empty group"),
        json!({})
    );
    assert!(
        serde_json::from_value::<RalphOptions>(json!({"method":"native/reconfigure"})).is_err()
    );
    let choices = json!({"prompt_path":"日本語.md", "done_marker":"DONE", "blocked_marker":"BLOCK", "waiting_marker":"WAIT"});
    let options: RalphOptions = serde_json::from_value(choices.clone()).expect("explicit choices");
    assert!(options.has_overrides());
    assert_eq!(
        serde_json::to_value(options).expect("exact choices"),
        choices
    );
}
