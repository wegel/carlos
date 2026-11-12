//! Keep one native compaction notice per attempt and fence obsolete context replies.

use serde_json::Value;

use super::activity::{ClaudeActivity, Compaction};
use crate::session::state::SessionState;

/// Let the client format the same notice from the stored native outcome.
pub(crate) struct CompactionNotice {
    pub(crate) key: String,
    pub(crate) result: CompactionResult,
}

/// Distinguish the two recognized outcomes from an unfinished or unknown outcome.
pub(crate) enum CompactionResult {
    Failed,
    Success,
    Pending,
}

impl SessionState {
    /// Update one attempt using the session's current helper generation.
    pub(crate) fn compact_claude_context(&mut self, message: &Value) -> CompactionNotice {
        let boundary = message["subtype"] == "compact_boundary";
        if self.claude.activity.starts_compaction(message, boundary) {
            self.claude.activity.compact_sequence += 1;
            self.claude.activity.compact = Some(Compaction {
                key: format!(
                    "compact:{}:{}",
                    self.claude.generation, self.claude.activity.compact_sequence
                ),
                boundary: None,
                result: None,
            });
        }
        let compact = self
            .claude
            .activity
            .compact
            .as_mut()
            .expect("compaction was created");
        if boundary {
            compact.boundary = message["uuid"].as_str().map(str::to_string);
            compact.result = Some("success".to_string());
        }
        if let Some(result) = message["compact_result"].as_str() {
            compact.result = Some(result.to_string());
        }
        let notice = CompactionNotice {
            key: compact.key.clone(),
            result: match compact.result.as_deref() {
                Some("failed") => CompactionResult::Failed,
                Some("success") => CompactionResult::Success,
                _ => CompactionResult::Pending,
            },
        };
        self.claude.activity.transient =
            (compact.result.is_none()).then(|| "Compacting".to_string());
        self.invalidate_claude_context();
        notice
    }
}

impl ClaudeActivity {
    fn starts_compaction(&self, message: &Value, boundary: bool) -> bool {
        let start = message["status"] == "compacting";
        let previous_finished = self
            .compact
            .as_ref()
            .is_some_and(|compact| compact.result.is_some());
        let new_boundary = boundary
            && self.compact.as_ref().is_some_and(|compact| {
                compact
                    .boundary
                    .as_deref()
                    .is_some_and(|id| Some(id) != message["uuid"].as_str())
            });
        self.compact.is_none() || (start && previous_finished) || new_boundary
    }
}
