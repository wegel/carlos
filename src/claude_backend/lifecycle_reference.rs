//! Test-only copy of main's ClaudeUiState methods, independent of the backend lifecycle.
//! Copied from main at 87a91739cf711ed6b69aaf606c83b6e723e4f750; only the owner name differs.

use std::collections::{BTreeMap, HashSet, VecDeque};

use serde_json::{json, Value};

/// Only the fields read by the copied main functions; no terminal or backend owner.
#[derive(Default)]
pub(super) struct Reference {
    pub(super) translation: Cursor,
    pub(super) prompts: Prompts,
    pending: VecDeque<String>,
    cycle: Vec<String>,
    results: Vec<(String, &'static str)>,
    interrupted: HashSet<String>,
    idle: bool,
}

#[derive(Default)]
pub(super) struct Cursor {
    current_turn_id: Option<String>,
}

#[derive(Default)]
pub(super) struct Prompts {
    pub(super) entries: BTreeMap<String, Prompt>,
}

#[derive(Default)]
pub(super) struct Prompt {
    pub(super) consumed: bool,
    pub(super) clear: bool,
}

impl Reference {
    /// Remember a prompt before its native stream can arrive.
    // main:src/app/claude_state.rs::ClaudeUiState::submitted
    pub(super) fn submitted(&mut self, id: &str) {
        if !self.pending.iter().any(|pending| pending == id) {
            self.pending.push_back(id.to_string());
        }
        self.idle = false;
    }

    /// Remove a prompt that the helper did not accept.
    // main:src/app/claude_state.rs::ClaudeUiState::rejected
    pub(super) fn rejected(&mut self, id: &str) {
        self.pending.retain(|pending| pending != id);
    }

    /// Bind a native reply to every prompt UUID it names.
    // main:src/app/claude_state.rs::ClaudeUiState::bind_reply
    pub(super) fn bind_reply(&mut self, message: &Value) {
        let ids = prompt_ids(message);
        if message["type"] == "system" && message["subtype"] == "init" {
            self.cycle.clear();
            self.translation.current_turn_id = None;
        }
        if !ids.is_empty() && ids != self.cycle {
            self.idle = false;
            self.cycle = ids;
        }
        if let Some(id) = self.cycle.last() {
            self.translation.current_turn_id = Some(id.clone());
        }
    }

    /// Capture the current prompt IDs before sending an interrupt.
    // main:src/app/claude_state.rs::ClaudeUiState::begin_interrupt
    pub(super) fn begin_interrupt(&mut self) -> Vec<String> {
        let mut ids = self.cycle.clone();
        if ids.is_empty() {
            ids = self
                .translation
                .current_turn_id
                .clone()
                .into_iter()
                .collect();
        }
        self.interrupted.extend(ids.iter().cloned());
        ids
    }

    /// Undo captured interrupt intent when the control request fails.
    // main:src/app/claude_state.rs::ClaudeUiState::rollback_interrupt
    pub(super) fn rollback_interrupt(&mut self, ids: &[String]) {
        for id in ids {
            self.interrupted.remove(id);
        }
    }

    /// Store terminal results until the native session also reports idle.
    // main:src/app/claude_state.rs::ClaudeUiState::result
    pub(super) fn result(&mut self, message: &Value) {
        let mut ids = prompt_ids(message);
        if ids.is_empty() {
            ids = self.cycle.clone();
        }
        for id in ids {
            if !self.pending.contains(&id) {
                continue;
            }
            if self
                .prompts
                .entries
                .get(&id)
                .is_some_and(|prompt| prompt.clear)
            {
                self.prompts.entries.remove(&id);
                self.pending.retain(|pending| pending != &id);
                continue;
            }
            let requested = self.interrupted.remove(&id);
            let status = if matches!(
                message["terminal_reason"].as_str(),
                Some("aborted_streaming" | "aborted_tools")
            ) {
                "interrupted"
            } else if message["is_error"] == true || message["subtype"] != "success" {
                "failed"
            } else if requested {
                "interrupted"
            } else {
                "completed"
            };
            if !self.results.iter().any(|(existing, _)| existing == &id) {
                self.results.push((id, status));
            }
        }
        self.translation.current_turn_id = None;
        self.cycle.clear();
    }

    /// Record whether the native main turn has stopped running.
    // main:src/app/claude_state.rs::ClaudeUiState::set_idle
    pub(super) fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
    }

    /// Release prompt completions once both result and idle have arrived.
    // main:src/app/claude_state.rs::ClaudeUiState::completions
    pub(super) fn completions(&mut self) -> Vec<String> {
        if !self.idle {
            return Vec::new();
        }
        self.results.drain(..).map(|(id, status)| {
            self.pending.retain(|pending| pending != &id);
            self.prompts.entries.remove(&id);
            json!({ "method": "turn/completed", "params": { "turn": { "id": id, "status": status } } }).to_string()
        }).collect()
    }

    /// Clear consumed work while preserving accepted inputs that native Claude has not run yet.
    // main:src/app/claude_state.rs::ClaudeUiState::reset_prompts
    pub(super) fn reset_prompts(&mut self, clear: Option<&str>) {
        self.prompts
            .entries
            .retain(|id, prompt| !prompt.consumed || Some(id.as_str()) == clear);
        if let Some(prompt) = clear.and_then(|id| self.prompts.entries.get_mut(id)) {
            prompt.clear = true;
        }
        self.pending
            .retain(|id| self.prompts.entries.contains_key(id));
        self.cycle.clear();
        self.results.clear();
        self.interrupted.clear();
        self.idle = false;
    }

    /// Fail pending prompts when the helper session closes.
    // main:src/app/claude_state.rs::ClaudeUiState::fail_pending
    pub(super) fn fail_pending(&mut self) -> Vec<String> {
        self.results.clear();
        self.results
            .extend(self.pending.drain(..).map(|id| (id, "failed")));
        self.idle = true;
        self.completions()
    }
}

// main:src/app/claude_state.rs::prompt_ids
/// Read only the exact prompt UUIDs that native Claude names as consumed.
pub(super) fn prompt_ids(message: &Value) -> Vec<String> {
    if let Some(ids) = message["user_message_uuids"].as_array() {
        return ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    message["user_message_uuid"]
        .as_str()
        .map(str::to_string)
        .into_iter()
        .collect()
}
