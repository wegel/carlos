//! Bind asynchronous labels to current transcript items without replacing commands.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc;

use serde_json::Value;

use super::state::AppState;
use super::summary_request::summary_input;
use crate::summaries::{Key, Request, SummaryEvent, SummaryService};

struct Binding {
    request: Request,
    original: String,
    label: Option<String>,
}

pub(super) struct SummaryState {
    pub enabled: bool,
    pub draft: bool,
    pub cwd: PathBuf,
    bindings: HashMap<String, Binding>,
    pending: VecDeque<String>,
    service: Option<SummaryService>,
    replaying: bool,
}

impl Default for SummaryState {
    fn default() -> Self {
        let enabled = super::tool_output_preferences::load_summary_enabled();
        Self {
            enabled,
            draft: enabled,
            cwd: PathBuf::new(),
            bindings: HashMap::new(),
            pending: VecDeque::new(),
            service: None,
            replaying: false,
        }
    }
}

impl SummaryState {
    pub fn replace_bindings(&mut self, other: Self) {
        if let Some(service) = &self.service {
            service.cancel_pending();
        }
        self.bindings = other.bindings;
        self.pending = other.pending;
        self.flush();
    }

    pub fn clear(&mut self) {
        if let Some(service) = &self.service {
            service.cancel_pending();
        }
        self.bindings.clear();
        self.pending.clear();
    }

    #[cfg(test)]
    pub fn retain_items(&mut self, items: &HashMap<String, usize>) {
        self.bindings.retain(|id, _| items.contains_key(id));
        self.pending.retain(|id| self.bindings.contains_key(id));
    }

    fn flush(&mut self) {
        if !self.enabled {
            return;
        }
        let Some(service) = &self.service else {
            return;
        };
        while let Some(id) = self.pending.front() {
            if let Some(binding) = self.bindings.get(id) {
                if !service.try_request(binding.request.clone()) {
                    break;
                }
            }
            self.pending.pop_front();
        }
    }

    pub fn label(&self, id: &str) -> Option<String> {
        self.enabled
            .then(|| self.bindings.get(id)?.label.clone())
            .flatten()
    }
}

impl AppState {
    /// Restore historical tool labels from cache without granting model generation to replayed events.
    pub(super) fn with_historical_tools<T>(&mut self, read: impl FnOnce(&mut Self) -> T) -> T {
        let previous = self.summaries.replaying;
        self.summaries.replaying = true;
        let result = read(self);
        self.summaries.replaying = previous;
        result
    }

    /// Only explicit live notification handlers may permit model generation.
    pub(super) fn observe_summary_item(&mut self, item: &Value, live: bool) {
        let Some(id) = item["id"].as_str() else {
            return;
        };
        if !super::summary_request::has_command(item) {
            return;
        }
        let Some(idx) = self.item_message_index(id) else {
            return;
        };
        if self.messages[idx].kind == super::MessageKind::Diff {
            return;
        }
        if !matches!(
            self.messages[idx].role,
            super::Role::ToolCall | super::Role::ToolOutput
        ) {
            return;
        }
        let id = self.item_key(id);
        let Some(input) = summary_input(item, &self.summaries.cwd) else {
            if self.summaries.bindings.remove(&id).is_some() {
                self.summaries.pending.retain(|pending| pending != &id);
                self.describe_tool_output(idx, item);
            }
            return;
        };
        if self
            .summaries
            .bindings
            .get(&id)
            .is_some_and(|b| b.request.key == input.key)
        {
            return;
        }
        let replaced = self.summaries.bindings.remove(&id).is_some();
        self.summaries.pending.retain(|pending| pending != &id);
        if replaced || self.messages[idx].tool_output.is_none() {
            self.describe_tool_output(idx, item);
        }
        let request = Request {
            key: input.key,
            command: input.command,
            location: input.location,
            generate: live && self.summaries.enabled && !self.summaries.replaying,
            entry: format!("{}:{id}", self.thread_id),
            file_based: input.file_based,
        };
        self.bind_summary_request(&id, idx, request);
    }

    fn bind_summary_request(&mut self, id: &str, idx: usize, request: Request) {
        let original = self.messages[idx]
            .tool_output
            .as_ref()
            .map(|t| t.summary.clone())
            .unwrap_or_else(|| format!("run {}", request.command));
        self.summaries.bindings.insert(
            id.to_owned(),
            Binding {
                request,
                original,
                label: None,
            },
        );
        self.summaries.pending.push_back(id.to_owned());
        self.summaries.flush();
    }

    pub(super) fn configure_summary_worker(&mut self, tx: mpsc::Sender<SummaryEvent>) {
        self.summaries.service = Some(SummaryService::spawn(tx, self.summaries.enabled));
        self.summaries.flush();
    }

    /// Check current content bindings, never saved row numbers, when a result arrives.
    pub(super) fn handle_summary_event(&mut self, event: SummaryEvent) {
        match event {
            SummaryEvent::Ready(key, entry, Some(label)) => self.apply_summary(key, &entry, label),
            SummaryEvent::Failed(entry, reason)
                if self.summaries.enabled
                    && (entry.is_empty()
                        || self
                            .summaries
                            .bindings
                            .values()
                            .any(|b| b.request.entry == entry)) =>
            {
                self.set_status(format!("AI tool label unavailable: {reason}"))
            }
            _ => {}
        }
        self.summaries.flush();
    }

    fn apply_summary(&mut self, key: Key, entry: &str, label: String) {
        let mut changed = Vec::new();
        for (id, binding) in &mut self.summaries.bindings {
            if binding.request.key != key || binding.request.entry != entry {
                continue;
            }
            binding.label = Some(match &binding.request.location.directory {
                Some(path) => format!("run {label} (in {path})"),
                None => format!("run {label}"),
            });
            if !self.summaries.enabled {
                continue;
            }
            if let Some(&idx) = self.item_to_message_index.get(id) {
                if self.messages[idx].kind != super::MessageKind::Diff {
                    if let Some(tool) = &mut self.messages[idx].tool_output {
                        tool.summary = binding.label.clone().unwrap_or_default();
                        tool.generated = true;
                        changed.push(idx);
                    }
                }
            }
        }
        for idx in changed {
            self.mark_transcript_dirty_from(idx);
        }
    }

    pub(super) fn apply_summary_setting(&mut self) {
        self.summaries.enabled = self.summaries.draft;
        if let Some(service) = &self.summaries.service {
            service.set_enabled(self.summaries.enabled);
        }
        let mut changed = Vec::new();
        for (id, binding) in &mut self.summaries.bindings {
            if !self.summaries.enabled {
                binding.request.generate = false;
            }
            if let Some(&idx) = self.item_to_message_index.get(id) {
                if let Some(tool) = &mut self.messages[idx].tool_output {
                    tool.generated = self.summaries.enabled && binding.label.is_some();
                    tool.summary = if self.summaries.enabled {
                        binding
                            .label
                            .clone()
                            .unwrap_or_else(|| binding.original.clone())
                    } else {
                        binding.original.clone()
                    };
                    changed.push(idx);
                }
            }
        }
        for idx in changed {
            self.mark_transcript_dirty_from(idx);
        }
        self.summaries.flush();
    }
}
