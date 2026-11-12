//! Apply worker labels and accepted preferences to current tool entries.

use std::sync::mpsc;

use serde_json::Value;

use super::entry_id::EntryId;
use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role};
use super::state::SessionState;
use super::summary_request::{has_command, summary_input, SummaryInput};
use super::summary_state::Binding;
use super::transcript_updates::SessionUpdate;
use crate::summaries::{Key, Request, SummaryEvent, SummaryService};

impl SessionState {
    /// Start the existing label worker and flush eligible pending requests.
    pub(crate) fn configure_summary_worker(&mut self, tx: mpsc::Sender<SummaryEvent>) {
        self.summaries.service = Some(SummaryService::spawn(tx, self.summaries.enabled));
        self.summaries.flush();
    }

    /// Check current content bindings, never saved row numbers, when a result arrives.
    pub(crate) fn handle_summary_event(&mut self, event: SummaryEvent) -> Vec<EntryId> {
        let mut changed = Vec::new();
        match event {
            SummaryEvent::Ready(key, entry, Some(label)) => {
                changed = self.apply_summary(key, &entry, label)
            }
            SummaryEvent::Failed(entry, reason)
                if self.summaries.enabled
                    && (entry.is_empty()
                        || self
                            .summaries
                            .bindings
                            .values()
                            .any(|b| b.request.binding == entry)) =>
            {
                self.set_status(format!("AI tool label unavailable: {reason}"))
            }
            _ => {}
        }
        self.summaries.flush();
        changed
    }

    fn apply_summary(&mut self, key: Key, entry: &str, label: String) -> Vec<EntryId> {
        let mut changed = Vec::new();
        for (id, binding) in &mut self.summaries.bindings {
            if binding.request.key != key || binding.request.binding != entry {
                continue;
            }
            binding.label = Some(match &binding.request.location.directory {
                Some(path) => format!("run {label} (in {path})"),
                None => format!("run {label}"),
            });
            if !self.summaries.enabled {
                continue;
            }
            if self
                .messages
                .entry(*id)
                .is_some_and(|message| message.kind != MessageKind::Diff)
            {
                if let Some(idx) =
                    self.messages
                        .set_label(*id, binding.label.clone().unwrap_or_default(), true)
                {
                    changed.push(self.messages[idx].id);
                }
            }
        }
        changed
    }

    /// Apply an accepted preference and restore literal labels when disabled.
    pub(crate) fn apply_summary_setting(&mut self, enabled: bool) -> Vec<EntryId> {
        self.summaries.enabled = enabled;
        if let Some(service) = &self.summaries.service {
            service.set_enabled(self.summaries.enabled);
        }
        let mut changed = Vec::new();
        for (id, binding) in &mut self.summaries.bindings {
            if !self.summaries.enabled {
                binding.request.generate = false;
            }
            let generated = self.summaries.enabled && binding.label.is_some();
            let label = if self.summaries.enabled {
                binding
                    .label
                    .clone()
                    .unwrap_or_else(|| binding.original.clone())
            } else {
                binding.original.clone()
            };
            if let Some(idx) = self.messages.set_label(*id, label, generated) {
                changed.push(self.messages[idx].id);
            }
        }
        self.summaries.flush();
        changed
    }
}

impl SessionState {
    /// Only an explicit live caller may grant model work to a command.
    pub(crate) fn observe_summary_item(
        &mut self,
        item: &Value,
        live: bool,
        key: &BackendEntryKey,
        cache_entry: String,
    ) -> SessionUpdate {
        let Some((id, idx)) = self.summary_entry(item, key) else {
            return SessionUpdate::default();
        };
        let Some(input) = summary_input(item, &self.summaries.cwd) else {
            return if self.summaries.bindings.remove(&id).is_some() {
                self.summaries.pending.retain(|pending| pending != &id);
                self.describe_tool_output(idx, item, Some(key))
            } else {
                SessionUpdate::default()
            };
        };
        if self
            .summaries
            .bindings
            .get(&id)
            .is_some_and(|b| b.request.key == input.key)
        {
            return SessionUpdate::default();
        }
        let replaced = self.summaries.bindings.remove(&id).is_some();
        self.summaries.pending.retain(|pending| pending != &id);
        let update = if replaced || self.messages[idx].tool_output.is_none() {
            self.describe_tool_output(idx, item, Some(key))
        } else {
            SessionUpdate::default()
        };
        let request = self.summary_request(id, input, live, cache_entry);
        self.bind_summary_request(id, idx, request);
        update
    }

    fn summary_entry(&self, item: &Value, key: &BackendEntryKey) -> Option<(EntryId, usize)> {
        item["id"].as_str()?;
        if !has_command(item) {
            return None;
        }
        let idx = self.item_message_index(key)?;
        let entry = &self.messages[idx];
        (entry.kind != MessageKind::Diff && matches!(entry.role, Role::ToolCall | Role::ToolOutput))
            .then_some((entry.id, idx))
    }

    // Keep the disk key unchanged; worker replies use a separate live binding.
    pub(crate) fn summary_cache_entry(&self, item: &str, scope: Option<(&str, usize)>) -> String {
        let key = match scope {
            Some((turn, part)) => format!("codex:{}", serde_json::json!([turn, item, part])),
            None => item.to_string(),
        };
        format!("{}:{key}", self.thread_id())
    }

    fn summary_request(
        &mut self,
        id: EntryId,
        input: SummaryInput,
        live: bool,
        entry: String,
    ) -> Request {
        self.summaries.revision = self
            .summaries
            .revision
            .checked_add(1)
            .expect("summary revision exhausted");
        Request {
            key: input.key,
            command: input.command,
            location: input.location,
            generate: live && self.summaries.enabled && !self.summaries.replaying,
            entry,
            binding: format!("{id:?}:{}", self.summaries.revision),
            file_based: input.file_based,
        }
    }

    fn bind_summary_request(&mut self, id: EntryId, idx: usize, request: Request) {
        let original = self.messages[idx]
            .tool_output
            .as_ref()
            .map(|t| t.summary.clone())
            .unwrap_or_else(|| format!("run {}", request.command));
        self.summaries.bindings.insert(
            id,
            Binding {
                request,
                original,
                label: None,
            },
        );
        self.summaries.pending.push_back(id);
        self.summaries.flush();
    }
}
