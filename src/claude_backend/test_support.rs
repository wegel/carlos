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

/// Owns native fixture cursors independently of any app or terminal widget.
#[derive(Default)]
pub(crate) struct FixtureNormalizer {
    reader: super::event_normalizer::EventNormalizer,
    handler: super::event_handler::EventHandler,
}

impl FixtureNormalizer {
    /// Decode legacy test inputs at the same backend boundary as controller stdout.
    pub(crate) fn lines(
        &mut self,
        parsed: &serde_json::Value,
        session: &str,
        generation: u64,
    ) -> Vec<String> {
        if parsed["method"] != "claude/controller" {
            return self.handler.handle(&parsed.to_string());
        }
        let params = &parsed["params"];
        self.reader.history.session = Some(session.to_string());
        let lines = self
            .reader
            .route(
                &params["frame"],
                params["requestMethod"].as_str(),
                &params["requestContext"],
                params["epoch"].as_u64().unwrap_or(0),
            )
            .expect("normalize native fixture")
            .into_iter()
            .map(|line| {
                let mut value: serde_json::Value =
                    serde_json::from_str(&line).expect("normalized JSON");
                value["params"]["generation"] = json!(generation);
                value.to_string()
            })
            .collect::<Vec<_>>();
        lines
            .iter()
            .flat_map(|line| self.handler.handle(line))
            .collect()
    }

    /// Capture the native interrupt intent in tests that previously reached into the app.
    pub(crate) fn begin_interrupt(&mut self) -> Vec<String> {
        self.handler.lifecycle.begin_interrupt()
    }

    /// Undo an interrupt that a fixture failed to send.
    pub(crate) fn rollback_interrupt(&mut self, ids: &[String]) {
        self.handler.lifecycle.rollback_interrupt(ids);
    }
}
