//! Keep every typed settings family routed after adding another host preference command.

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::Command;
use serde_json::{json, Value};

use super::prepare;
use crate::retry::Class;
use crate::transport::wire::decode;

#[test]
fn context_native_and_label_settings_reach_their_own_ordinary_work() {
    let cases = [
        (
            "settings/context/set",
            json!({"choice":{"kind":"tokens","tokens":500000}}),
        ),
        (
            "settings/set",
            json!({"expected":"a".repeat(64),"change":{},"save_default":true}),
        ),
        (
            "settings/tool-labels/set",
            json!({"expected":"b".repeat(64),"enabled":false,"save_default":true}),
        ),
    ];
    for (index, (method, extra)) in cases.into_iter().enumerate() {
        let (_, _, work, recovery, class) = prepare(command(method, extra)).expect("settings work");
        assert!(matches!(class, Class::Ordinary));
        assert!(recovery.is_none());
        match (index, work) {
            (0, EngineCommand::ContextSet { choice, .. }) => {
                assert_eq!(Option::<u64>::from(choice), Some(500_000))
            }
            (
                1,
                EngineCommand::SettingsSet {
                    expected,
                    save_default,
                    ..
                },
            ) => {
                assert_eq!(expected.to_string(), "a".repeat(64));
                assert!(save_default);
            }
            (
                2,
                EngineCommand::ToolLabelsSet {
                    expected,
                    enabled,
                    save_default,
                    ..
                },
            ) => {
                assert_eq!(expected.to_string(), "b".repeat(64));
                assert!(!enabled);
                assert!(save_default);
            }
            _ => panic!("settings command reached another mutation family"),
        }
    }
}

fn command(method: &str, extra: Value) -> Command {
    let mut params = json!({
        "session":"11111111111111111111111111111111",
        "operation":{"client":"22222222222222222222222222222222","id":"1"},
    });
    params
        .as_object_mut()
        .expect("params")
        .extend(extra.as_object().expect("fields").clone());
    let frame =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .expect("frame");
    decode(&frame).expect("typed settings request").command
}
