//! Converts accepted controller frames into ordered common messages and named Claude facts.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use serde_json::{json, Value};

use super::event_history::HistoryDecoder;
use super::types::ClaudeTranslationState;

/// Owns only the reader's live and history translator cursors, never the caller's turn state.
pub(super) struct EventNormalizer {
    pub(super) live: ClaudeTranslationState,
    pub(super) history: HistoryDecoder,
    pub(super) epoch: u64,
    closed: bool,
    explicit_closed: bool,
    pub(super) reset_key: Option<String>,
}

impl Default for EventNormalizer {
    fn default() -> Self {
        static STREAM: AtomicU64 = AtomicU64::new(1);
        Self {
            live: ClaudeTranslationState {
                stream_instance_seq: STREAM.fetch_add(1, Ordering::Relaxed),
                ..Default::default()
            },
            history: Default::default(),
            epoch: 0,
            closed: false,
            explicit_closed: false,
            reset_key: None,
        }
    }
}

impl EventNormalizer {
    /// Decode one accepted frame; callers send each returned line through EventSender.
    pub(super) fn route(
        &mut self,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
        epoch: u64,
    ) -> Result<Vec<String>> {
        self.explicit_closed |= frame["event"] == "closed";
        let process = frame["type"] == "callback"
            || frame["event"] == "callback_cancelled"
            || matches!(
                request,
                Some("start" | "capabilities" | "task.stop" | "usage.optional" | "context")
            )
            || super::conversation::process_message(&frame["message"]);
        if epoch < self.epoch && !process {
            return Ok(Vec::new());
        }
        if self.closed && Self::requires_open_session(frame) {
            return Ok(Vec::new());
        }
        let mut output = match frame["type"].as_str() {
            Some("reply") => self.reply(frame, request, context)?,
            Some("callback") => vec![fact(
                "claude/callback",
                json!({"callbackId":frame["callbackId"],"kind":frame["kind"],"payload":frame["payload"]}),
            )],
            Some("event") if frame["event"] == "sdk" => self.sdk(&frame["message"], epoch)?,
            Some("event") => self.event(frame),
            _ => Vec::new(),
        };
        for line in &mut output {
            let mut value: Value = serde_json::from_str(line)?;
            value["params"]["epoch"] = json!(epoch);
            *line = value.to_string();
        }
        if request == Some("sessions.read") && frame["ok"] == true {
            for line in output {
                self.history
                    .delivery
                    .extend(super::fact_transfer::parts(line)?);
            }
            return Ok(self.next_history());
        }
        Ok(output)
    }

    /// Identify frames that cannot produce facts after this native session closes.
    pub(super) fn requires_open_session(frame: &Value) -> bool {
        matches!(frame["type"].as_str(), Some("callback" | "reply")) || frame["event"] == "sdk"
    }

    /// Release one bounded history piece before waiting for the client's next page request.
    pub(super) fn next_history(&mut self) -> Vec<String> {
        if self.closed {
            self.history.delivery.clear();
            return Vec::new();
        }
        let mut output = Vec::new();
        while let Some(line) = self.history.delivery.pop_front() {
            let part = serde_json::from_str::<Value>(&line)
                .is_ok_and(|value| value["method"] == "claude/historyFactPart");
            output.push(line);
            if part {
                break;
            }
        }
        output
    }

    /// Translate a transport failure without deciding which prompts it completes.
    pub(super) fn failure(&mut self, message: &str) -> Vec<String> {
        self.closed = true;
        self.history = HistoryDecoder::default();
        vec![fact(
            "claude/error",
            json!({"error":{"message":message},"epoch":self.epoch}),
        )]
    }

    /// Translate EOF after the synchronous callers have been woken.
    pub(super) fn exited(&mut self, message: &str, report_error: bool) -> Vec<String> {
        if self.explicit_closed && !report_error {
            return Vec::new();
        }
        self.failure(message)
    }

    /// Report whether a terminal native fact closed this helper generation.
    pub(super) fn is_closed(&self) -> bool {
        self.closed
    }

    fn reply(
        &mut self,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
    ) -> Result<Vec<String>> {
        let mut output = Vec::new();
        let mut result = frame["result"].clone();
        if frame["ok"] == true
            && request == Some("sessions.info")
            && (result["sessionId"].as_str().is_none()
                || result["sessionId"].as_str() != self.history.session.as_deref())
        {
            return Ok(vec![fact(
                "claude/controlReply",
                json!({"ok":false,"error":{"message":"The requested Claude session was not found"},"requestMethod":request,"requestContext":context}),
            )]);
        }
        if frame["ok"] == true && request == Some("sessions.read") {
            if let Err(error) = self.history.page(&result, &mut output) {
                // main keeps earlier records, reports the read error, and closes only the UI query.
                output.push(fact("claude/controlReply", json!({"ok":false,"error":{"message":error.to_string()},"requestMethod":request,"requestContext":context})));
                return Ok(output);
            }
            if let Some(object) = result.as_object_mut() {
                object.remove("messages");
                object.remove("chunk");
            }
        }
        if frame["ok"] == true && request == Some("context") {
            if let (Some(used), Some(max)) = (
                result["totalTokens"].as_u64(),
                result["rawMaxTokens"]
                    .as_u64()
                    .or_else(|| result["maxTokens"].as_u64())
                    .filter(|max| *max > 0),
            ) {
                output.push(fact("thread/tokenUsage/updated", json!({"tokenUsage":{"total":{"totalTokens":used},"modelContextWindow":max},"claudeContext":result,"requestContext":context})));
                return Ok(output);
            }
        }
        output.push(fact("claude/controlReply", json!({"ok":frame["ok"],"result":result,"error":frame["error"],"requestMethod":request,"requestContext":context})));
        Ok(output)
    }

    fn event(&mut self, frame: &Value) -> Vec<String> {
        let method = match frame["event"].as_str() {
            Some("stderr") => "claude/stderr",
            Some("callback_cancelled") => "claude/callbackCancelled",
            Some("error" | "transport_error") => "claude/error",
            Some("closed") => "claude/closed",
            _ => return Vec::new(),
        };
        self.explicit_closed |= method == "claude/closed";
        let output = vec![fact(
            method,
            json!({"text":frame["text"],"callbackId":frame["callbackId"],"dialogKind":frame["dialogKind"],"error":frame["error"],"reason":frame["reason"]}),
        )];
        if method == "claude/closed"
            || (method == "claude/error" && frame["error"]["code"] != "unsupported_dialog")
        {
            self.closed = true;
        }
        output
    }
}

/// Build a named backend fact in the existing method/params envelope.
pub(super) fn fact(method: &str, params: Value) -> String {
    json!({"method":method,"params":params}).to_string()
}
