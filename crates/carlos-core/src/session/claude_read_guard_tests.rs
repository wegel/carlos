//! Check context token/revision ownership, malformed counts and unavailable readers through Core.

use std::time::Instant;

use serde_json::{json, Value};

use super::claude::ClaudePhase;
use super::claude_context_contract_tests::{new_session, poll};
use super::context_usage::ContextUsage;

#[test]
fn context_token_and_revision_guards_settle_only_the_owned_read() {
    for (request, expected, retry) in [
        (json!({"contextToken":1,"revision":0}), Some(17), false),
        (json!({"contextToken":2,"revision":0}), None, false),
        (json!({"contextToken":1,"revision":99}), None, true),
        (json!({"revision":0}), Some(17), false),
        (
            json!({"contextToken":"bad","revision":"bad"}),
            Some(17),
            false,
        ),
    ] {
        let mut core = new_session();
        let mut requests = Vec::new();
        let now = Instant::now();
        assert!(!poll(&mut core, &mut requests, now));
        assert_eq!(requests[0], json!({"revision":0,"contextToken":1}));
        core.receive_claude_context(
            &json!({"ok":true,"result":{"totalTokens":17,"rawMaxTokens":100}}),
            &request,
        );
        assert_eq!(
            core.context_usage().map(|usage| usage.used),
            expected,
            "{request}"
        );
        poll(&mut core, &mut requests, now);
        assert_eq!(requests.len(), if retry { 2 } else { 1 });
    }
}

#[test]
fn context_reply_defaults_keep_known_counts_when_the_reply_cannot_supply_valid_counts() {
    for (ok, result, expected, fresh) in [
        (
            true,
            json!({"totalTokens":7,"rawMaxTokens":100,"maxTokens":200}),
            (7, 100),
            true,
        ),
        (
            true,
            json!({"totalTokens":7,"maxTokens":200}),
            (7, 200),
            true,
        ),
        (
            true,
            json!({"totalTokens":7,"rawMaxTokens":0,"maxTokens":200}),
            (42, 1000),
            false,
        ),
        (
            true,
            json!({"totalTokens":-1,"maxTokens":200}),
            (42, 1000),
            false,
        ),
        (true, json!({"maxTokens":200}), (42, 1000), false),
        (
            false,
            json!({"totalTokens":7,"maxTokens":200}),
            (42, 1000),
            false,
        ),
    ] {
        let mut core = new_session();
        core.set_context_usage(Some(ContextUsage {
            used: 42,
            max: 1000,
        }));
        core.receive_claude_context(&json!({"ok":ok,"result":result}), &Value::Null);
        let usage = core.context_usage().expect("last known counts remain");
        assert_eq!((usage.used, usage.max), expected);
        assert_eq!(core.claude().context_fresh(), fresh);
    }
}

#[test]
fn unavailable_context_readers_never_send_or_wake() {
    for (enabled, phase, label) in [
        (false, ClaudePhase::Ready, None),
        (true, ClaudePhase::Starting, None),
        (true, ClaudePhase::Closed, None),
        (true, ClaudePhase::Ready, Some("Compacting")),
    ] {
        let mut core = new_session();
        core.set_claude_enabled_for_test(enabled);
        core.set_claude_phase_for_test(phase);
        core.record_claude_status(&json!({"label":label}));
        let mut requests = Vec::new();
        let now = Instant::now();
        assert!(!poll(&mut core, &mut requests, now));
        assert!(core.claude_context_wait(now).is_none());
        assert!(requests.is_empty());
    }
}
