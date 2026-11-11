//! Renders child-agent text separately from the main assistant and its human prompt cycle.

use std::collections::HashMap;

use serde_json::Value;

use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

use super::claude_question_state::safe_label;
use super::models::{MessageKind, Role};
use super::notifications::handle_server_message_line;
use super::state::AppState;

/// Keeps each child's stream cursor separate so snapshots do not repeat its output.
#[derive(Default)]
pub(super) struct ClaudeChildren {
    streams: HashMap<String, ChildStream>,
    sequence: u64,
}

struct ChildStream {
    translation: ClaudeTranslationState,
    caller: Option<String>,
    completed: bool,
    sequence: u64,
}

/// Forward actual child output without creating a main assistant turn or a human prompt.
pub(super) fn receive(app: &mut AppState, message: &Value) -> Result<(), &'static str> {
    let id = message["agent_id"]
        .as_str()
        .or_else(|| message["parent_tool_use_id"].as_str())
        .unwrap_or("unknown");
    let snapshot = (message["type"] == "assistant").then(|| {
        format!(
            "child-snapshot:{}",
            message["uuid"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| blake3::hash(message.to_string().as_bytes()).to_string())
        )
    });
    if snapshot
        .as_ref()
        .is_some_and(|key| app.is_item_completed(key))
    {
        return Ok(());
    }
    app.claude.children.prepare(id, message)?;
    let stream = &mut app
        .claude
        .children
        .streams
        .get_mut(id)
        .expect("child stream was prepared")
        .translation;
    let Ok(output) = translate_claude_line(stream, &message.to_string()) else {
        return Ok(());
    };
    if let Some(key) = snapshot {
        app.mark_item_completed(&key);
    }
    for line in output.lines {
        render_line(app, id, message, &line);
    }
    Ok(())
}

impl ClaudeChildren {
    /// Keep recently finished cursors while allowing a long session to run more children.
    pub(super) fn finish(&mut self, id: &str, caller: Option<&str>) {
        for (key, stream) in &mut self.streams {
            if key == id || caller.is_some_and(|caller| stream.caller.as_deref() == Some(caller)) {
                stream.completed = true;
            }
        }
    }

    fn prepare(&mut self, id: &str, message: &Value) -> Result<(), &'static str> {
        if !self.streams.contains_key(id) && self.streams.len() >= 128 {
            let oldest = self
                .streams
                .iter()
                .filter(|(_, stream)| stream.completed)
                .min_by_key(|(_, stream)| stream.sequence)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.streams.remove(&id);
            } else {
                return Err("More than 128 child streams are active; Claude stopped to keep its output bounded");
            }
        }
        self.sequence += 1;
        let stream = self.streams.entry(id.to_string()).or_insert_with(|| {
            let mut translation = ClaudeTranslationState::default();
            translation.stream_instance_seq = self.sequence;
            ChildStream {
                translation,
                caller: message["parent_tool_use_id"].as_str().map(str::to_string),
                completed: false,
                sequence: self.sequence,
            }
        });
        stream.sequence = self.sequence;
        if message["type"] == "result" {
            stream.completed = true;
        }
        Ok(())
    }
}

fn render_line(app: &mut AppState, id: &str, message: &Value, line: &str) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let item_id = value["params"]["itemId"]
        .as_str()
        .or_else(|| value["params"]["item"]["id"].as_str())
        .unwrap_or("");
    let key = format!("child:{}:{id}:{item_id}", app.claude.generation);
    match value["method"].as_str() {
        Some("item/agentMessage/delta" | "item/reasoning/summaryTextDelta") => {
            if !app.has_item_mapping(&key) {
                let index = app.append_message(Role::ToolOutput, prefix(id, message));
                app.put_item_mapping(&key, index);
            }
            if let Some(text) = value["params"]["delta"].as_str() {
                app.upsert_command_output_delta(&key, text);
            }
        }
        Some("item/completed")
            if matches!(
                value["params"]["item"]["type"].as_str(),
                Some("agentMessage" | "reasoning")
            ) =>
        {
            let item = &value["params"]["item"];
            let text = item["text"]
                .as_str()
                .or_else(|| item["summary"][0]["text"].as_str())
                .unwrap_or("");
            app.upsert_mapped_message(
                &key,
                Role::ToolOutput,
                format!("{}{text}", prefix(id, message)),
                MessageKind::Plain,
                None,
            );
        }
        Some(method)
            if method.starts_with("item/")
                && value["params"]["item"]["type"] != "agentMessage"
                && value["params"]["item"]["type"] != "reasoning" =>
        {
            let _ = handle_server_message_line(app, line);
        }
        _ => {}
    }
}

fn prefix(id: &str, message: &Value) -> String {
    format!(
        "Agent {} (caller {}):\n",
        safe_label(id),
        safe_label(
            message["parent_tool_use_id"]
                .as_str()
                .unwrap_or("native task")
        )
    )
}
