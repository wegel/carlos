//! Preserve context-read deadlines, stale counts and helper/model guards at the daemon's Core seam.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::backend_test_support::normalize;
use super::claude::ClaudePhase;
use super::context_usage::ContextUsage;
use super::runtime_settings::RuntimeSetting;
use super::state::SessionState;
use crate::claude_backend::FixtureNormalizer;

/// Construct a ready native root without a client editor or host preference read.
pub(super) fn new_session() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_claude();
    core
}

/// Capture only actual prepared native requests, using the same slot as the daemon timer.
pub(super) fn poll(core: &mut SessionState, requests: &mut Vec<Value>, now: Instant) -> bool {
    let (redraw, request) = core.prepare_claude_context(now);
    requests.extend(request);
    redraw
}

fn sdk(core: &mut SessionState, native: &mut FixtureNormalizer, message: Value) {
    let _ = normalize(
        core,
        native,
        &json!({"method":"claude/controller","params":{
        "frame":{"type":"event","event":"sdk","message":message}}}),
    );
}

fn answer(core: &mut SessionState, context: &Value, used: u64) {
    core.receive_claude_context(
        &json!({"ok":true,"result":{"totalTokens":used,"maxTokens":90000,"rawMaxTokens":100000}}),
        context,
    );
}

fn fail_prepared(core: &mut SessionState, now: Instant) -> bool {
    let (redraw, request) = core.prepare_claude_context(now);
    let token = request.expect("actual prepared native write")["contextToken"]
        .as_u64()
        .expect("captured token");
    core.fail_claude_context_request(token, now) | redraw
}

#[test]
fn repeated_running_and_child_usage_keep_known_root_context_and_one_pending_read() {
    let mut core = new_session();
    let mut native = FixtureNormalizer::default();
    let mut requests = Vec::new();
    let now = Instant::now();
    poll(&mut core, &mut requests, now);
    let request = requests[0].clone();
    answer(&mut core, &request, 42000);
    let revision = core.claude().activity().context_revision();
    for _ in 0..3 {
        sdk(
            &mut core,
            &mut native,
            json!({"type":"system","subtype":"session_state_changed","state":"running"}),
        );
        poll(&mut core, &mut requests, now);
    }
    assert_eq!(core.claude().activity().context_revision(), revision);
    assert_eq!(
        core.context_usage(),
        Some(ContextUsage {
            used: 42000,
            max: 100000
        })
    );
    assert_eq!(requests.len(), 1);
    poll(&mut core, &mut requests, now + Duration::from_secs(2));
    assert_eq!(requests.len(), 2);
    sdk(
        &mut core,
        &mut native,
        json!({"type":"assistant","agent_id":"child","message":{"usage":{"input_tokens":999999},"content":[]}}),
    );
    assert_eq!(core.context_usage().expect("root context").used, 42000);
}

#[test]
fn stalled_read_marks_known_counts_stale_without_more_requests_and_recovers_when_it_settles() {
    let mut core = new_session();
    let mut native = FixtureNormalizer::default();
    let mut requests = Vec::new();
    let now = Instant::now();
    poll(&mut core, &mut requests, now);
    let first = requests[0].clone();
    answer(&mut core, &first, 42000);
    assert!(
        core.claude_context_wait(now + Duration::from_secs(60))
            .is_none(),
        "idle snapshots do not poll forever"
    );
    sdk(
        &mut core,
        &mut native,
        json!({"type":"system","subtype":"session_state_changed","state":"running"}),
    );
    poll(&mut core, &mut requests, now + Duration::from_secs(2));
    let request = requests[1].clone();
    core.set_claude_context_fresh_for_test(true);
    assert!(poll(&mut core, &mut requests, now + Duration::from_secs(8)));
    assert!(!core.claude().context_fresh());
    assert!(core
        .claude_context_wait(now + Duration::from_secs(8))
        .is_none());
    poll(&mut core, &mut requests, now + Duration::from_secs(60));
    assert_eq!(requests.len(), 2);
    answer(&mut core, &request, 43000);
    assert!(core.claude().context_fresh());
    assert_eq!(
        core.context_usage()
            .expect("new snapshot replaces old")
            .used,
        43000
    );
    poll(&mut core, &mut requests, now + Duration::from_secs(61));
    assert_eq!(requests.len(), 3);
}

#[test]
fn obsolete_compaction_and_model_reads_release_the_slot_without_restoring_old_counts() {
    let mut core = new_session();
    let mut native = FixtureNormalizer::default();
    let mut requests = Vec::new();
    let now = Instant::now();
    poll(&mut core, &mut requests, now);
    let before = requests[0].clone();
    sdk(
        &mut core,
        &mut native,
        json!({"type":"system","subtype":"status","status":"compacting"}),
    );
    answer(&mut core, &before, 99000);
    assert!(core.context_usage().is_none());
    sdk(
        &mut core,
        &mut native,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"success"}),
    );
    poll(&mut core, &mut requests, now);
    let fresh = requests.last().expect("new read").clone();
    answer(&mut core, &fresh, 10000);
    assert_eq!(
        core.context_usage().expect("post-compact count").used,
        10000
    );
    core.set_claude_observed_model_for_test("old");
    core.refresh_claude_context();
    poll(&mut core, &mut requests, now + Duration::from_secs(2));
    let old_model = requests.last().expect("model read").clone();
    sdk(
        &mut core,
        &mut native,
        json!({"type":"system","subtype":"init","model":"new"}),
    );
    answer(&mut core, &old_model, 80000);
    assert!(
        core.context_usage().is_none(),
        "same-batch old-model reply is fenced before tick"
    );
    poll(&mut core, &mut requests, now);
    assert_eq!(requests.len(), 4);
}

#[test]
fn optional_read_failure_keeps_known_counts_and_retries_without_failing_chat() {
    let mut core = new_session();
    let mut requests = Vec::new();
    let now = Instant::now();
    poll(&mut core, &mut requests, now);
    let first = requests[0].clone();
    answer(&mut core, &first, 42000);
    core.refresh_claude_context();
    poll(&mut core, &mut requests, now + Duration::from_secs(2));
    let request = requests[1].clone();
    core.receive_claude_context(
        &json!({"ok":false,"error":{"message":"optional unavailable"}}),
        &request,
    );
    assert_eq!(
        core.context_usage().expect("last known retained").used,
        42000
    );
    assert!(!core.claude().context_fresh());
    assert!(core.messages().is_empty());
    poll(&mut core, &mut requests, now);
    assert_eq!(requests.len(), 2);
    poll(&mut core, &mut requests, now + Duration::from_secs(8));
    assert_eq!(requests.len(), 3);
    core.invalidate_claude_context();
    assert!(core.context_usage().is_none());
}

#[test]
fn helper_replacement_retires_pending_read_and_old_generation_cannot_install_branch_counts() {
    let mut core = new_session();
    let mut native = FixtureNormalizer::default();
    let mut requests = Vec::new();
    poll(&mut core, &mut requests, Instant::now());
    let old = requests[0].clone();
    let _ = normalize(
        &mut core,
        &mut native,
        &json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"branch"}}),
    );
    let _ = normalize(
        &mut core,
        &mut native,
        &json!({"method":"claude/controller","params":{"generation":1,"requestMethod":"context","requestContext":old,
        "frame":{"type":"reply","ok":true,"result":{"totalTokens":999,"rawMaxTokens":1000}}}}),
    );
    assert!(core.context_usage().is_none());
    core.set_claude_phase_for_test(ClaudePhase::Ready);
    poll(&mut core, &mut requests, Instant::now());
    assert_eq!(requests.len(), 2);
}

#[test]
fn immediate_write_failure_and_tick_only_model_change_request_a_visible_redraw() {
    let mut core = new_session();
    core.set_context_usage(Some(ContextUsage {
        used: 42000,
        max: 100000,
    }));
    core.set_claude_context_fresh_for_test(true);
    core.record_turn_started("busy");
    assert!(fail_prepared(&mut core, Instant::now()));
    assert!(!core.claude().context_fresh());
    assert!(core.context_usage().is_some());
    core.record_runtime_choice(RuntimeSetting::Model, Some("changed".into()));
    assert!(fail_prepared(&mut core, Instant::now()));
    assert!(core.context_usage().is_none());
}
