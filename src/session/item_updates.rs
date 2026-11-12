//! Update command labels and normalize reasoning while retaining native entry identity.

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role};
use super::reasoning::normalize_reasoning_summary_stream;
use super::state::SessionState;
use super::transcript_updates::SessionUpdate;

impl SessionState {
    /// Read the parsed native command label for a scoped item.
    pub(crate) fn command_override(&self, key: &BackendEntryKey) -> Option<String> {
        self.command_render_overrides.get(key).cloned()
    }

    /// Preserve generated/failure/body/path facts when a native command label arrives.
    pub(crate) fn set_command_override(
        &mut self,
        key: &BackendEntryKey,
        summary: String,
    ) -> SessionUpdate {
        self.command_render_overrides
            .insert(key.clone(), summary.clone());
        if let Some(index) = self.item_message_index(key) {
            if let Some(tool) = &self.messages[index].tool_output {
                let generated = tool.generated;
                self.messages
                    .set_label(self.messages[index].id, summary.clone(), generated);
                return self.dirty_entry(index);
            }
        }
        self.update_mapped_message(key, Role::ToolCall, Some(summary), MessageKind::Plain, None)
            .1
    }

    /// Append reasoning text using the same completion guard as other streamed text.
    pub(crate) fn upsert_reasoning_delta(
        &mut self,
        key: &BackendEntryKey,
        delta: &str,
    ) -> SessionUpdate {
        self.upsert_text_delta(key, Role::Reasoning, delta, false)
    }

    /// Normalize after appending, preserving the second incremental dirty call.
    pub(crate) fn normalize_reasoning_at(&mut self, index: usize) -> SessionUpdate {
        let message = &self.messages[index];
        let text = normalize_reasoning_summary_stream(&message.text);
        self.messages.update(
            index,
            message.role,
            Some(text),
            message.kind,
            message.file_path.clone(),
        );
        self.dirty_entry(index)
    }
}
