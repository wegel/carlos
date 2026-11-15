//! Keep bounded native notice keys, exact escaping and usage defaults in Core.

use serde_json::json;

use super::{named, new_session, show, usage};

#[test]
fn notice_keys_keep_fifo_order_and_do_not_refresh_on_duplicates() {
    let mut core = new_session("session");
    for index in 0..128 {
        show(&mut core, format!("key-{index}"), format!("notice {index}"));
    }
    show(&mut core, "key-0".into(), "duplicate".into());
    show(&mut core, "key-128".into(), "notice 128".into());
    show(&mut core, "key-0".into(), "returned".into());
    assert_eq!(core.messages().len(), 130);
    assert_eq!(core.messages()[129].text, "returned");
    core.receive_claude_reset(
        &json!({"new_conversation_id":"reset"}),
        &mut Default::default(),
    );
    show(&mut core, "key-0".into(), "still duplicate".into());
    assert!(core.messages().is_empty());
    core.switch_claude_session(2, "branch", &mut Default::default());
    show(&mut core, "key-0".into(), "new generation".into());
    assert_eq!(core.messages()[0].text, "new generation");
}

#[test]
fn native_warning_defaults_and_escaping_remain_exact() {
    let mut core = new_session("session");
    for facts in [
        json!({"subtype":"plugin_install","status":"failed"}),
        json!({"subtype":"hook_response"}),
        json!({"subtype":"informational","content":""}),
        json!({"subtype":"init","plugin_errors":[{}]}),
        json!({"subtype":"informational","noticeKey":"warning","content":"line\n\u{001b}"}),
        json!({"subtype":"informational","noticeKey":"warning","content":"duplicate"}),
        json!({"subtype":"plugin_install","status":"ok"}),
        json!({"subtype":"unknown"}),
    ] {
        let _ = named(&mut core, "claude/notice", &facts);
    }
    let rows: Vec<_> = core.messages().iter().map(|m| m.text.as_str()).collect();
    assert_eq!(
        rows,
        [
            "Plugin unknown failed: unknown error",
            "Hook unknown (unknown):  ",
            "Plugin unknown: could not load (unknown path)",
            "line\n\\u{1b}"
        ]
    );
}

#[test]
fn usage_notices_skip_invalid_rows_and_bound_input_before_filtering() {
    let mut core = new_session("session");
    let row = json!({"percent":25.0,"scope":{"model":{"display_name":"model\nname"},
        "surface":{"display_name":"ignored"}}});
    let mut rows = vec![
        json!({"percent":-1}),
        json!({"percent":101}),
        json!({"percent":"25"}),
        row,
    ];
    rows.extend((4..129).map(|_| json!({"percent":100})));
    usage(
        &mut core,
        &json!({"uuid":"usage","usage_report":{"rate_limits":{"limits":rows}}}),
    );
    let lines: Vec<_> = core.messages()[0].text.lines().collect();
    assert_eq!(lines.len(), 125);
    assert_eq!(
        lines[0],
        "quota model\\nname: 25.0% used; resets unknown (unknown)"
    );
    assert_eq!(lines[124], "quota : 100.0% used; resets unknown (unknown)");
}
