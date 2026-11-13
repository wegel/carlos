//! Update command labels and normalize reasoning while retaining native entry identity.

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role};
use super::reasoning::normalize_reasoning_summary_stream;
use super::state::SessionState;

impl SessionState {
    /// Read the parsed native command label for a scoped item.
    pub fn command_override(&self, key: &BackendEntryKey) -> Option<String> {
        self.command_render_overrides.get(key).cloned()
    }

    /// Preserve generated/failure/body/path facts when a native command label arrives.
    pub fn set_command_override(&mut self, key: &BackendEntryKey, summary: String) {
        self.command_render_overrides
            .insert(key.clone(), summary.clone());
        if let Some(index) = self.item_message_index(key) {
            if let Some(tool) = &self.messages[index].tool_output {
                let generated = tool.generated;
                self.messages
                    .set_label(self.messages[index].id, summary.clone(), generated);
                return;
            }
        }
        self.update_mapped_message(key, Role::ToolCall, Some(summary), MessageKind::Plain, None);
    }

    /// Append reasoning text using the same completion guard as other streamed text.
    pub fn upsert_reasoning_delta(&mut self, key: &BackendEntryKey, delta: &str) {
        self.upsert_text_delta(key, Role::Reasoning, delta, false)
    }

    /// Normalize streamed reasoning through the event-recording store.
    pub fn normalize_reasoning_at(&mut self, index: usize) {
        let message = &self.messages[index];
        let text = normalize_reasoning_summary_stream(&message.text);
        self.messages.update(
            index,
            message.role,
            Some(text),
            message.kind,
            message.file_path.clone(),
        );
    }
}
