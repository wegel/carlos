//! Queues prompt ownership before writing to the helper; the caller registers it when handled.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::commands::Commands;
use super::event_queue::EventSender;
use super::transport::ControllerTransport;
use crate::backend::{PromptOrigin, PromptPriority};

impl Commands {
    /// Announce prompt ownership before any native result can name its UUID.
    pub(super) fn submit(
        &self,
        params: &Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        if params.get("model").is_some() || params.get("effort").is_some() {
            self.apply_runtime_settings(params["model"].as_str(), params["effort"].as_str())?;
        }
        let text = params["input"][0]["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .context("missing prompt text")?;
        let uuid = params["clientUserMessageId"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let input_kind = (origin == PromptOrigin::Human).then_some(match priority {
            PromptPriority::Steer => "steer",
            PromptPriority::Queue => "prompt",
        });
        let (transport, events) = self.prompt_transport()?;
        let submitted = json!({ "method": "claude/promptSubmitted", "params": { "uuid": uuid,"text":text,"inputKind":input_kind,"origin": match origin { PromptOrigin::Human => "human", PromptOrigin::Automatic => "automatic" },"priority":match priority { PromptPriority::Steer => "now", PromptPriority::Queue => "later" } } });
        events.send(submitted.to_string())?;
        let mut prompt = json!({ "uuid": uuid, "text": text,
            "origin": { "kind": match origin { PromptOrigin::Human => "human", PromptOrigin::Automatic => "auto-continuation" } },
            "priority": match priority { PromptPriority::Steer => "now", PromptPriority::Queue => "later" } });
        if let Some(kind) = input_kind {
            prompt["inputKind"] = json!(kind);
        }
        let result =
            transport.call_guarded("prompt", prompt, timeout, self.write_permission(false));
        if let Err(error) = result {
            let _ = events.send(
                json!({ "method": "claude/promptRejected", "params": { "uuid": uuid } })
                    .to_string(),
            );
            return Err(error);
        }
        Ok(json!({ "result": { "turn": { "id": uuid }, "inputKind":input_kind } }).to_string())
    }

    fn prompt_transport(&self) -> Result<(Arc<ControllerTransport>, EventSender)> {
        self.check()?;
        let transport = self.transport.clone();
        let events = self.events_tx.for_generation(transport.generation);
        Ok((transport, events))
    }
}
