//! Keep malformed, lost and mismatched native acknowledgements explicitly uncertain.

use carlos_proto::submissions::OperationState;
use serde_json::{json, Value};

use super::task_stop_reply::{claude, codex};

fn context() -> Value {
    json!({"operation":"taskStop","clientOperation":{"client":"a".repeat(32),"id":"1"},
        "task":"b".repeat(64),"expectedRun":"c".repeat(64)})
}

#[test]
fn codex_task_stop_errors_use_codes_and_malformed_success_stays_uncertain() {
    let mut frame = json!({"requestContext":context(),"requestMethod":"thread/backgroundTerminals/terminate",
        "response":{"result":{"terminated":true}}});
    assert!(matches!(
        codex(&frame).expect("correlated reply").state,
        OperationState::Accepted { .. }
    ));
    frame["response"]["result"]["terminated"] = json!(false);
    assert!(matches!(
        codex(&frame).expect("already absent is acknowledged").state,
        OperationState::Accepted { .. }
    ));
    frame["response"] = json!({"result":{}});
    assert!(matches!(
        codex(&frame).expect("missing native acknowledgement").state,
        OperationState::Uncertain { .. }
    ));
    frame["response"] = json!({"error":{"code":-32602,"message":"native validation error"}});
    assert!(matches!(
        codex(&frame).expect("validation reply").state,
        OperationState::Rejected { .. }
    ));
    frame["response"] = json!({"error":{"code":-32000,"message":"native validation error"}});
    assert!(matches!(
        codex(&frame)
            .expect("same words cannot prove non-dispatch")
            .state,
        OperationState::Uncertain { .. }
    ));
    frame["requestMethod"] = json!("arbitrary/native/method");
    assert!(codex(&frame).is_none());
}

#[test]
fn claude_task_stop_success_requires_original_nonempty_identity_and_errors_remain_private_data() {
    let mut context = json!({"taskStopContext":context(),"taskId":"task","runId":"original"});
    let success = json!({"ok":true,"result":{"taskId":"task","stopRequested":true}});
    assert!(matches!(
        claude(&success, &context)
            .expect("original acknowledgement")
            .state,
        OperationState::Accepted { .. }
    ));
    let mismatch = json!({"ok":true,"result":{"taskId":"other","stopRequested":true}});
    assert!(matches!(
        claude(&mismatch, &context).expect("wrong task").state,
        OperationState::Uncertain { .. }
    ));
    let failure =
        json!({"ok":false,"error":{"code":"stale_task_run","message":"stale\u{1b}[31m run"}});
    let OperationState::Rejected { message, recovery } = claude(&failure, &context)
        .expect("helper rejected before SDK")
        .state
    else {
        panic!("definite refusal")
    };
    assert!(!message.contains('\u{1b}'));
    assert!(recovery.is_none());
    context["taskId"] = Value::Null;
    let malformed = json!({"ok":true,"result":{"stopRequested":true}});
    assert!(matches!(
        claude(&malformed, &context)
            .expect("missing task is not a match")
            .state,
        OperationState::Uncertain { .. }
    ));
}
