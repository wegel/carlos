//! Keeps accepted native UUIDs and authors so queued human prompts survive a transcript reset.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::session::state::SessionState;

/// Keeps only pending prompts; completed transcript text remains in the transcript itself.
#[derive(Default, Debug)]
pub(in crate::session) struct ClaudePrompts {
    pub(in crate::session) entries: BTreeMap<String, Prompt>,
}

#[derive(Debug)]
/// Retains the exact text accepted by native Claude without guessing who wrote it.
pub struct Prompt {
    pub(in crate::session) text: String,
    pub(in crate::session) human: bool,
    pub(in crate::session) steer: bool,
    pub(in crate::session) priority: String,
    pub(in crate::session) consumed: bool,
    pub(in crate::session) clear: bool,
}

impl ClaudePrompts {
    /// Register one accepted input before native events can consume its UUID.
    pub(in crate::session) fn register(&mut self, params: &Value) -> Result<(), &'static str> {
        let (Some(id), Some(text)) = (params["uuid"].as_str(), params["text"].as_str()) else {
            return Ok(());
        };
        if self.entries.contains_key(id) {
            return Ok(());
        }
        if self.entries.len() >= 1024
            || self
                .entries
                .values()
                .map(|entry| entry.text.len())
                .sum::<usize>()
                + text.len()
                > 8 * 1024 * 1024
        {
            return Err("Claude accepted-input history exceeded its bounded queue");
        }
        self.entries.insert(
            id.to_string(),
            Prompt {
                text: text.to_string(),
                human: params["origin"] == "human",
                steer: params["origin"] == "human" && params["inputKind"] == "steer",
                priority: params["priority"].as_str().unwrap_or("later").to_string(),
                consumed: false,
                clear: false,
            },
        );
        Ok(())
    }
}

impl SessionState {
    /// Retain accepted text before native facts can consume its UUID.
    pub fn register_claude_prompt(&mut self, params: &Value) -> Result<(), &'static str> {
        self.claude.prompts.register(params)
    }

    /// Forget only the named rejected, retired, or completed prompt.
    pub(crate) fn remove_claude_prompt(&mut self, id: &str) {
        self.claude.prompts.entries.remove(id);
    }

    /// Preserve unconsumed accepted inputs and the native clear command.
    pub fn reset_claude_prompts(&mut self, clear: Option<&str>) {
        self.claude
            .prompts
            .entries
            .retain(|id, prompt| !prompt.consumed || Some(id.as_str()) == clear);
        if let Some(prompt) = clear.and_then(|id| self.claude.prompts.entries.get_mut(id)) {
            prompt.clear = true;
        }
    }

    /// Fail accepted UUIDs in sorted order, then any unregistered active turn.
    pub(crate) fn fail_claude_prompts(&mut self) {
        let mut ids: Vec<String> = self.claude.prompts.entries.keys().cloned().collect();
        if let Some(active) = &self.active_turn_id() {
            if !ids.contains(active) {
                ids.push(active.clone());
            }
        }
        for id in ids {
            if self.record_turn_completed(&id, crate::session::models::TurnStatus::Failed) {
                self.clear_turn_boundary();
                self.set_status("turn failed");
            }
        }
        self.claude.prompts.entries.clear();
    }
}

impl SessionState {
    /// Iterate accepted prompts in their existing UUID sort order.
    pub fn claude_prompts(&self) -> impl Iterator<Item = (&str, &Prompt)> {
        self.claude
            .prompts
            .entries
            .iter()
            .map(|(id, prompt)| (id.as_str(), prompt))
    }
}

impl Prompt {
    /// Borrow the exact accepted prompt text.
    #[cfg(any(test, feature = "test-support"))]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Report whether the native source marked this prompt human.
    #[cfg(any(test, feature = "test-support"))]
    pub fn human(&self) -> bool {
        self.human
    }
    /// Report whether native Claude already consumed the accepted input.
    pub fn consumed(&self) -> bool {
        self.consumed
    }
    /// Report whether this accepted input is the invisible native clear command.
    pub fn clear(&self) -> bool {
        self.clear
    }
    /// Borrow the native queue priority, including unknown native values.
    pub fn priority(&self) -> &str {
        &self.priority
    }
}
