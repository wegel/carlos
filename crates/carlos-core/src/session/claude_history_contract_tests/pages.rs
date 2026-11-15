//! Keep native page metadata, child records and chunk bounds at the Core history decoder.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use super::{control, named, native, new_session, record};
use crate::claude_backend::FixtureNormalizer;
use crate::session::claude::ClaudePhase;

#[test]
fn foreign_record_fails_after_retaining_earlier_rows_and_metadata() {
    let mut core = new_session("session");
    let mut decoder = FixtureNormalizer::default();
    native(
        &mut core,
        &mut decoder,
        "sessions.read",
        json!({"messages":[
        record("session","first","human","earlier"),record("other","wrong","human","foreign")]}),
    );
    assert_eq!(core.claude().phase(), ClaudePhase::Closed);
    assert_eq!(core.thread_id(), "session");
    assert_eq!(core.claude_history().records().len(), 1);
    assert_eq!(core.messages()[0].text, "earlier");
    assert_eq!(
        core.messages()[1].text,
        "Claude history belongs to a different session"
    );
    assert_eq!(core.input_history().history_len(), 1);
    assert_eq!(core.claude_history().records()[0]["opaque"]["kept"], true);
}

#[test]
fn child_records_keep_metadata_but_hide_prompt_and_transcript_rows() {
    let mut core = new_session("session");
    let mut decoder = FixtureNormalizer::default();
    let mut child = record("session", "child", "human", "hidden");
    child["message"]["parent_tool_use_id"] = json!("task");
    native(
        &mut core,
        &mut decoder,
        "sessions.read",
        json!({"messages":[child,
        record("session","visible","unknown","legacy")],"nextOffset":null}),
    );
    assert_eq!(core.claude_history().records().len(), 2);
    assert_eq!(core.claude_history().records()[0]["opaque"]["kept"], true);
    assert_eq!(core.messages().len(), 1);
    assert_eq!(core.messages()[0].text, "[Source unknown] legacy");
    assert_eq!(core.claude_history().unknown().len(), 1);
    assert_eq!(core.claude_history().unknown()[0].uuid(), "visible");
    assert_eq!(core.input_history().history_len(), 0);
}

#[test]
fn native_chunks_reject_wrong_offset_oversized_total_and_declared_overrun() {
    for (offset, total, bytes) in [(1, 2, "x"), (0, 16 * 1024 * 1024 + 1, "x"), (0, 1, "xx")] {
        let mut core = new_session("session");
        let mut decoder = FixtureNormalizer::default();
        native(
            &mut core,
            &mut decoder,
            "sessions.read",
            json!({"messages":[record("session","first","human","earlier")],"nextOffset":1}),
        );
        native(
            &mut core,
            &mut decoder,
            "sessions.read",
            json!({"chunk":{"offset":offset,"totalBytes":total,"data":STANDARD.encode(bytes)}}),
        );
        assert_eq!(core.claude().phase(), ClaudePhase::Closed);
        assert_eq!(core.claude_history().records().len(), 1);
        assert_eq!(core.messages()[0].text, "earlier");
        assert_eq!(
            core.messages()[1].text,
            "Claude history chunk exceeds its declared size or arrived out of order"
        );
    }
}

#[test]
fn history_controls_keep_snapshot_fields_and_default_chunk_offset() {
    let mut core = new_session("session");
    for (offset, expected) in [(None, 0), (Some(json!(7)), 7), (Some(json!("bad")), 0)] {
        let mut result = json!({"nextOffset":9,"snapshotId":{"opaque":true}});
        if let Some(offset) = offset {
            result["chunkOffset"] = offset;
        }
        assert_eq!(
            core.claude_history_control("sessions.read", &result)
                .expect("next page"),
            (
                "sessions.read",
                json!({"sessionId":"session","offset":9,"snapshotId":{"opaque":true},"chunkOffset":expected})
            )
        );
    }
    for id in [Value::Null, json!(7), json!("foreign")] {
        assert!(core
            .claude_history_control("sessions.info", &json!({"sessionId":id}))
            .is_err());
    }
    control(
        named(
            &mut core,
            "claude/controlReply",
            &json!({"ok":true,"requestMethod":"sessions.info","result":{
                "sessionId":"session","customTitle":"  named  "}
            }),
        ),
        "sessions.read",
        json!({"sessionId":"session"}),
    );
    assert_eq!(core.thread_name().as_deref(), Some("named"));
    assert_eq!(core.status(), "Reading Claude history");
    assert_eq!(
        core.claude_history_control("sessions.read", &json!({}))
            .expect("start after history"),
        ("start", json!({"sessionId":"session"}))
    );
}
