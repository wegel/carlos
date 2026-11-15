//! Keep native quota reads and guards in Core while Device owns its minute redraw clock.

use std::time::{Duration, Instant};

use serde_json::json;

use super::{ClaudePhase, QuotaReadKey};
use crate::backend::{BackendClient, BackendKind};
use crate::claude_backend::receiver_client;
use crate::grok_backend::receiver_test_support;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

fn new_session() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_claude();
    core
}

#[test]
fn quota_optional_reads_and_countdown_draws_keep_their_independent_deadlines() {
    let mut core = new_session();
    let now = Instant::now();
    let mut requests = Vec::new();
    for (seconds, reads, native_wait) in [
        (0, 1, 300),
        (59, 1, 241),
        (60, 1, 240),
        (299, 1, 1),
        (300, 2, 300),
        (359, 2, 241),
    ] {
        let clock = now + Duration::from_secs(seconds);
        if let Some(params) = core.prepare_claude_quota(clock) {
            let key = QuotaReadKey::from_context(&params).expect("original native read identity");
            core.fail_claude_quota_request(key);
            requests.push(params);
        }
        assert_eq!(requests.len(), reads);
        assert_eq!(
            core.claude().quota().wait(clock),
            Some(Duration::from_secs(native_wait))
        );
    }
    let scope = requests[0]["quotaScope"]
        .as_str()
        .expect("original native scope");
    assert_eq!(scope.len(), 64);
    assert_eq!(
        requests[0],
        json!({"revision":0,"quotaRead":1,"quotaScope":scope})
    );
    assert_eq!(
        requests[1],
        json!({"revision":0,"quotaRead":2,"quotaScope":scope})
    );
    assert!(
        core.messages().is_empty(),
        "optional write failure stays silent"
    );
}

#[test]
fn quota_timer_guards_skip_nonclaude_and_unready_helpers() {
    for (kind, phase) in [
        (BackendKind::Codex, ClaudePhase::Ready),
        (BackendKind::Grok, ClaudePhase::Ready),
        (BackendKind::Claude, ClaudePhase::Starting),
        (BackendKind::Claude, ClaudePhase::Closed),
    ] {
        let mut core = new_session();
        core.set_claude_phase_for_test(phase);
        match kind {
            BackendKind::Codex => assert_unavailable(&mut core, &TestBackend::default()),
            BackendKind::Grok => {
                let (client, _events) = receiver_test_support::client(Vec::new());
                assert_unavailable(&mut core, &client);
            }
            BackendKind::Claude => {
                let (client, _events) = receiver_client(Vec::new(), json!({}), 1024);
                assert_unavailable(&mut core, &client);
            }
        }
    }
}

fn assert_unavailable(core: &mut SessionState, native: &dyn BackendClient) {
    let now = Instant::now();
    assert!(core.claude().quota().wait(now).is_none());
    core.tick_claude_quota(native, now);
    assert!(
        core.claude().quota().wait(now).is_none(),
        "an unavailable backend cannot prepare a read or leave a wake deadline"
    );
    assert!(core.messages().is_empty());
}
