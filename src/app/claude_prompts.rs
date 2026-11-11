//! Keeps accepted native UUIDs and authors so queued human prompts survive a transcript reset.

use std::collections::BTreeMap;

use serde_json::Value;

use super::models::Role;
use super::state::AppState;

/// Keeps only pending prompts; completed transcript text remains in the transcript itself.
#[derive(Default)]
pub(super) struct ClaudePrompts {
    pub(super) entries: BTreeMap<String, Prompt>,
    pub(super) queue: bool,
}

/// Retains the exact text accepted by native Claude without guessing who wrote it.
pub(super) struct Prompt {
    pub(super) text: String,
    pub(super) human: bool,
    pub(super) priority: String,
    pub(super) consumed: bool,
    pub(super) clear: bool,
}

impl ClaudePrompts {
    /// Register one accepted input before native events can consume its UUID.
    pub(super) fn register(&mut self, params: &Value) -> Result<(), &'static str> {
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
                priority: params["priority"].as_str().unwrap_or("later").to_string(),
                consumed: false,
                clear: false,
            },
        );
        Ok(())
    }
}

/// Associate a local echo with its native UUID before the next SDK frame is handled.
pub(super) fn attach_echo(app: &mut AppState, id: &str) {
    if let Some(index) = app.messages.len().checked_sub(1) {
        app.put_item_mapping(&format!("claude-prompt:{id}"), index);
        app.input_history.record_turn_id(index, id);
    }
}

/// Show surviving accepted prompts once, when native Claude actually consumes them.
pub(super) fn consume(app: &mut AppState, ids: &[String]) {
    for id in ids {
        let Some(prompt) = app.claude.prompts.entries.get_mut(id) else {
            continue;
        };
        prompt.consumed = true;
        let (text, human, clear) = (prompt.text.clone(), prompt.human, prompt.clear);
        if clear {
            continue;
        }
        let key = format!("claude-prompt:{id}");
        if !app.has_item_mapping(&key) {
            let role = if human { Role::User } else { Role::System };
            let index = app.append_message(
                role,
                if human {
                    text.clone()
                } else {
                    format!("Automatic prompt: {text}")
                },
            );
            app.put_item_mapping(&key, index);
            if human {
                app.record_input_history(&text, Some(index));
                app.input_history.record_turn_id(index, id);
            }
        }
        app.record_turn_started(id);
    }
}
