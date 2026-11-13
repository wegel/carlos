//! Completes accepted Claude prompts only after their native result and idle facts agree.

use std::collections::{HashSet, VecDeque};

use anyhow::{bail, Result};
use serde_json::{json, Value};

/// Tracks only accepted native prompt UUIDs until both result and idle have arrived.
#[derive(Default)]
pub(super) struct Lifecycle {
    pending: VecDeque<String>,
    cycle: Vec<String>,
    results: Vec<(String, &'static str)>,
    interrupted: HashSet<String>,
    current: Option<String>,
    consumed: HashSet<String>,
    clear: HashSet<String>,
    idle: bool,
}

impl Lifecycle {
    /// Remember a prompt when the caller handles its accepted-input fact.
    pub(super) fn submitted(&mut self, id: &str) -> Result<()> {
        if !self.pending.iter().any(|pending| pending == id) {
            if self.pending.len() >= 1024 {
                bail!("Claude accepted-input history exceeded its bounded queue");
            }
            self.pending.push_back(id.to_string());
        }
        self.idle = false;
        Ok(())
    }

    /// Remove a prompt that the helper did not accept.
    pub(super) fn rejected(&mut self, id: &str) {
        self.pending.retain(|pending| pending != id);
        self.consumed.remove(id);
        self.clear.remove(id);
        self.interrupted.remove(id);
    }

    /// Bind a native reply to every prompt UUID it names.
    pub(super) fn bind_reply(&mut self, message: &Value) -> Vec<String> {
        let ids = prompt_ids(message);
        let started = ids
            .iter()
            .filter(|id| {
                self.pending.contains(id)
                    && !self.consumed.contains(*id)
                    && !self.clear.contains(*id)
            })
            .cloned()
            .collect();
        self.consumed
            .extend(ids.iter().filter(|id| self.pending.contains(id)).cloned());
        if message["init"] == true {
            self.cycle.clear();
            self.current = None;
        }
        if !ids.is_empty() && ids != self.cycle {
            self.idle = false;
            self.cycle = ids;
        }
        if let Some(id) = self.cycle.last() {
            self.current = Some(id.clone());
        }
        started
    }

    /// Capture the prompt the caller has handled before its synchronous interrupt.
    pub(super) fn begin_interrupt(&mut self) -> Vec<String> {
        let mut ids = self.cycle.clone();
        if ids.is_empty() {
            ids = self.current.clone().into_iter().collect();
        }
        self.interrupted.extend(ids.iter().cloned());
        ids
    }

    /// Undo captured intent when the synchronous control fails.
    pub(super) fn rollback_interrupt(&mut self, ids: &[String]) {
        for id in ids {
            self.interrupted.remove(id);
        }
    }

    /// Store terminal results until the native session also reports idle.
    pub(super) fn result(&mut self, message: &Value) -> Vec<String> {
        let mut ids = prompt_ids(message);
        if ids.is_empty() {
            ids = self.cycle.clone();
        }
        let mut retired = Vec::new();
        for id in ids {
            if !self.pending.contains(&id) {
                continue;
            }
            if self.clear.contains(&id) {
                self.rejected(&id);
                retired.push(id);
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
        self.current = None;
        self.cycle.clear();
        retired
    }

    /// Record whether the native main turn has stopped running.
    pub(super) fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
    }

    /// Release results at the handler position where native idle and result agree.
    pub(super) fn completions(&mut self) -> Vec<String> {
        if !self.idle {
            return Vec::new();
        }
        self.results.drain(..).map(|(id, status)| {
            self.pending.retain(|pending| pending != &id);
            self.consumed.remove(&id);
            self.clear.remove(&id);
            json!({ "method": "turn/completed", "params": { "turn": { "id": id, "status": status } } }).to_string()
        }).collect()
    }

    /// Clear consumed work while preserving accepted inputs that native Claude has not run yet.
    pub(super) fn reset_prompts(&mut self, clear: Option<&str>) {
        self.pending
            .retain(|id| !self.consumed.contains(id) || Some(id.as_str()) == clear);
        self.consumed.retain(|id| self.pending.contains(id));
        self.clear.retain(|id| self.pending.contains(id));
        if let Some(id) = clear.filter(|id| self.pending.iter().any(|pending| pending == id)) {
            self.clear.insert(id.to_string());
        }
        self.cycle.clear();
        self.current = None;
        self.results.clear();
        self.interrupted.clear();
        self.idle = false;
    }

    /// Fail pending prompts when the helper session closes.
    pub(super) fn fail_pending(&mut self) -> Vec<String> {
        self.interrupted.clear();
        self.results.clear();
        self.results
            .extend(self.pending.drain(..).map(|id| (id, "failed")));
        self.idle = true;
        self.completions()
    }
}

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
