//! Send submitted Codex callback answers and controls through the existing backend.

use serde_json::{json, Value};

use super::callback_commands::CallbackRetired;
use super::callbacks::CallbackKey;
use crate::backend::BackendClient;
use crate::session::approval_state::{ApprovalChoice, ApprovalRequestKind};
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;
use crate::session::text::safe_text;

/// Distinguish an unavailable interrupt target from a failed backend request.
pub enum CallbackInterrupt {
    Ignored,
    Requested,
    NoTurn,
    Failed(String),
}

/// Report the backend outcome while the app applies local focus and notices.
pub enum CallbackAnswered {
    Retired(CallbackRetired),
    Interrupted(CallbackInterrupt),
}

impl SessionState {
    /// Validate accepted form content or send the already converted native answer.
    pub fn answer_codex_callback(
        &mut self,
        client: &dyn BackendClient,
        card_id: EntryId,
        id: &Value,
        answer: Value,
        choice: ApprovalChoice,
    ) -> Result<Option<CallbackAnswered>, String> {
        let Some(pending) = self
            .approval()
            .pending()
            .filter(|p| p.entry_id() == card_id)
        else {
            return Ok(None);
        };
        let kind = pending.facts().kind;
        if choice == ApprovalChoice::Cancel && kind == ApprovalRequestKind::CodexQuestion {
            return Ok(Some(CallbackAnswered::Interrupted(
                self.interrupt_codex_callback(client, id),
            )));
        }
        let Some(entry) = self.codex_callbacks.get(id) else {
            return Ok(None);
        };
        if kind == ApprovalRequestKind::CodexForm
            && answer["action"] == "accept"
            && entry.params["mode"] == "form"
        {
            self.begin_approval_answer(card_id);
            self.validate_codex_answer(client, card_id, id, &answer)?;
            return Ok(None);
        }
        self.send_codex_answer(client, id, answer)
            .map(|retired| Some(CallbackAnswered::Retired(retired)))
    }

    fn validate_codex_answer(
        &mut self,
        client: &dyn BackendClient,
        card_id: EntryId,
        id: &Value,
        answer: &Value,
    ) -> Result<(), String> {
        let entry = self.codex_callbacks.get_mut(id).expect("active callback");
        if entry.checking {
            return Ok(());
        }
        entry.revision = entry.revision.saturating_add(1);
        entry.candidate = Some(answer.clone());
        entry.candidate_card = Some(card_id);
        entry.checking = true;
        client
            .validate_form(
                id.clone(),
                entry.params["requestedSchema"].clone(),
                Some(answer["content"].clone()),
                entry.revision,
            )
            .map_err(|e| e.to_string())
    }

    /// Resolve the original submitted card before a deferred validation reply is sent.
    pub fn send_codex_callback(
        &mut self,
        client: &dyn BackendClient,
        card_id: EntryId,
        answer: Value,
    ) -> Result<Option<CallbackRetired>, String> {
        let Some(pending) = self.submitted_approval_answer(card_id) else {
            return Ok(None);
        };
        let id = pending.request_id.clone();
        self.send_codex_answer(client, &id, answer).map(Some)
    }

    fn send_codex_answer(
        &mut self,
        client: &dyn BackendClient,
        id: &Value,
        answer: Value,
    ) -> Result<CallbackRetired, String> {
        client
            .respond(id, answer)
            .map_err(|error| format!("Could not send answer: {error}"))?;
        let retired = self.retire_codex_callback(id);
        self.set_status("Answer sent");
        Ok(retired)
    }

    /// Interrupt the request's own turn without treating acknowledgment as resolution.
    pub fn interrupt_codex_callback(
        &mut self,
        client: &dyn BackendClient,
        id: &Value,
    ) -> CallbackInterrupt {
        let Some(entry) = self.codex_callbacks.get(id) else {
            return CallbackInterrupt::Ignored;
        };
        if entry.interrupting {
            return CallbackInterrupt::Ignored;
        }
        let params = json!({"threadId":entry.params["threadId"],"turnId":entry.params["turnId"]});
        if !params["threadId"].is_string() || !params["turnId"].is_string() {
            return CallbackInterrupt::NoTurn;
        }
        match client.request_native_control("turn/interrupt", params) {
            Ok(()) => {
                self.edit_codex_callback(id);
                if let Some(entry) = self.codex_callbacks.get_mut(id) {
                    entry.interrupting = true;
                }
                CallbackInterrupt::Requested
            }
            Err(error) => CallbackInterrupt::Failed(error.to_string()),
        }
    }

    /// Clear interrupt progress only on the failed reply's exact thread and turn.
    pub(crate) fn codex_callback_control_reply(
        &mut self,
        params: &Value,
    ) -> Option<(Vec<CallbackKey>, String)> {
        if params["requestMethod"] != "turn/interrupt" || params["response"]["error"].is_null() {
            return None;
        }
        let target = &params["requestParams"];
        let mut keys = Vec::new();
        for (key, entry) in &mut self.codex_callbacks.entries {
            if entry.params["threadId"] == target["threadId"]
                && entry.params["turnId"] == target["turnId"]
            {
                entry.interrupting = false;
                keys.push(key.clone());
            }
        }
        let error = format!(
            "Codex could not interrupt: {}. The request is still pending",
            safe_text(
                params["response"]["error"]["message"]
                    .as_str()
                    .unwrap_or("unknown error")
            )
        );
        Some((keys, error))
    }
}
