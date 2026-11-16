//! Freeze flat settings envelopes and field order used by canonical retry bytes before record reuse.

use crate::commands::Command;

#[test]
fn settings_mutations_keep_their_flat_v1_bytes_and_default_fields() {
    let session = "1".repeat(32);
    let client = "2".repeat(32);
    let revision = "a".repeat(64);
    let cases = [
        (
            "settings/context/set",
            r#""choice":{"kind":"tokens","tokens":500000}"#.to_owned(),
        ),
        (
            "settings/tool-labels/set",
            format!(r#""expected":"{revision}","enabled":false,"save_default":true"#),
        ),
        (
            "settings/set",
            format!(
                r#""expected":"{revision}","change":{{"model":null,"effort":null,"summary":null}},"save_default":true"#
            ),
        ),
    ];
    for (method, fields) in cases {
        let expected = format!(
            r#"{{"method":"{method}","params":{{"session":"{session}","operation":{{"client":"{client}","id":"1"}},{fields}}}}}"#
        );
        let command: Command = serde_json::from_str(&expected).expect("existing flat v1 command");
        assert_eq!(
            serde_json::to_string(&command).expect("canonical command"),
            expected
        );
        if method != "settings/context/set" {
            let omitted = expected.replace(",\"save_default\":true", "");
            let inherited: Command =
                serde_json::from_str(&omitted).expect("existing omitted default");
            assert_eq!(
                serde_json::to_string(&inherited).expect("explicit false default"),
                expected.replace("\"save_default\":true", "\"save_default\":false")
            );
        }
    }
}
