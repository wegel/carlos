//! Helpers for historical transcript and session-error regression tests.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::translate::translate_claude_line;
use super::types::ClaudeTranslationState;

/// Translate fixture messages with distinct live stream IDs.
pub(crate) fn collect_live_forwarded_lines_for_test(lines: &[&str]) -> Vec<String> {
    static STREAM: AtomicU64 = AtomicU64::new(1);
    let mut state = ClaudeTranslationState {
        stream_instance_seq: STREAM.fetch_add(1, Ordering::Relaxed),
        ..ClaudeTranslationState::default()
    };
    lines
        .iter()
        .flat_map(|line| {
            translate_claude_line(&mut state, line)
                .expect("translate")
                .lines
        })
        .collect()
}

/// Build a diagnostic for a resumed session that does not match its requested ID.
pub(crate) fn session_misroute_event(expected: &str, actual: &str) -> String {
    json!({ "method": "error", "params": {
        "message": format!("Claude resumed session {actual} but {expected} was requested; session stopped"),
        "expectedSessionId": expected, "actualSessionId": actual } }).to_string()
}
