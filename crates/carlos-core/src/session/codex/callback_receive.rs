//! Accept bounded Codex callback records before the app chooses local focus.

use serde_json::Value;

use super::approval_parsing::pending_approval_from_request;
use super::callback_questions::CallbackEditor;
use super::callbacks::{CallbackKey, CodexCallback, CodexCallbacks};
use crate::session::approval_state::{ApprovalQueue, ApprovalRequestDraft, ApprovalRequestKind};
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;
use crate::session::text::{safe_label, safe_text};

/// Return a plain callback result without constructing or returning a card.
pub(crate) enum CallbackReceived {
    Ignored,
    Rejected {
        id: Value,
        code: i64,
        message: String,
    },
    Accepted {
        key: CallbackKey,
        card_id: EntryId,
        editor: CallbackEditor,
        blocking: bool,
        preflight: Option<Value>,
    },
}

impl SessionState {
    /// Check a native callback and enqueue its card through the existing sole write point.
    pub(crate) fn receive_codex_callback(
        &mut self,
        method: &str,
        frame: &Value,
    ) -> CallbackReceived {
        let id = frame["id"].clone();
        if self.codex_callbacks.contains(&id) {
            return CallbackReceived::Ignored;
        }
        if !supported_method(method) {
            return reject(id, -32601, "Unsupported native request method");
        }
        let Some(key) = CallbackKey::new(&id) else {
            return reject(id, -32600, "Invalid callback ID");
        };
        let params = &frame["params"];
        if !params.is_object() {
            return reject(id, -32602, "Callback params must be an object");
        }
        let bytes = params.to_string().len() + key.0.len();
        if exceeds_callback_limits(&self.codex_callbacks, bytes) {
            return reject(id, -32000, "Pending callback count or size limit exceeded");
        }
        let (pending, editor) = match build_request(method, params, id) {
            Ok(built) => built,
            Err(error) => return reject(frame["id"].clone(), -32602, &error),
        };
        let blocking = method != "item/tool/requestUserInput"
            || params["isBlocking"].as_bool().unwrap_or(true);
        self.codex_callbacks.entries.insert(
            key.clone(),
            CodexCallback {
                params: params.clone(),
                blocking,
                revision: 0,
                candidate: None,
                candidate_card: None,
                checking: false,
                schema_error: None,
                interrupting: false,
                bytes,
            },
        );
        let card_id = self.enqueue_approval(pending, ApprovalQueue::Codex);
        self.set_status("Codex has a pending request");
        let preflight = (method == "mcpServer/elicitation/request" && params["mode"] == "form")
            .then(|| params["requestedSchema"].clone());
        CallbackReceived::Accepted {
            key,
            card_id,
            editor,
            blocking,
            preflight,
        }
    }
}

fn reject(id: Value, code: i64, message: &str) -> CallbackReceived {
    CallbackReceived::Rejected {
        id,
        code,
        message: message.into(),
    }
}

fn exceeds_callback_limits(callbacks: &CodexCallbacks, bytes: usize) -> bool {
    let retained = callbacks
        .entries
        .values()
        .map(|entry| entry.bytes)
        .sum::<usize>();
    callbacks.entries.len() >= 64 || bytes > 128 * 1024 || retained + bytes > 4 * 1024 * 1024
}

fn supported_method(method: &str) -> bool {
    matches!(
        method,
        "item/tool/requestUserInput"
            | "mcpServer/elicitation/request"
            | "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "execCommandApproval"
            | "applyPatchApproval"
    )
}

fn build_request(
    method: &str,
    params: &Value,
    id: Value,
) -> Result<(ApprovalRequestDraft, CallbackEditor), String> {
    validate_owner(method, params)?;
    let (mut pending, draft) = if matches!(
        method,
        "item/tool/requestUserInput" | "mcpServer/elicitation/request"
    ) {
        super::callback_questions::build(method, params, id).map_err(|error| error.to_string())?
    } else {
        let pending =
            pending_approval_from_request(method, params.as_object().expect("checked params"), id)
                .ok_or_else(|| "Malformed native approval".to_string())?;
        (pending, CallbackEditor::empty())
    };
    if pending.kind != ApprovalRequestKind::Permissions && pending.requested_permissions.is_none() {
        pending.requested_permissions = Some(params.clone());
    }
    pending.title = safe_label(&pending.title);
    pending.detail_lines = pending
        .detail_lines
        .iter()
        .map(|line| safe_text(line))
        .collect();
    if let Some(thread) = params["threadId"]
        .as_str()
        .or(params["conversationId"].as_str())
    {
        if !pending
            .detail_lines
            .iter()
            .any(|line| line.starts_with("Thread:"))
        {
            pending
                .detail_lines
                .insert(0, format!("Thread: {}", safe_label(thread)));
        }
    }
    Ok((pending, draft))
}

fn validate_owner(method: &str, params: &Value) -> Result<(), String> {
    let keys: &[&str] = if method == "mcpServer/elicitation/request" {
        &["threadId", "serverName"]
    } else if matches!(method, "execCommandApproval" | "applyPatchApproval") {
        &["conversationId", "callId"]
    } else {
        &["threadId", "turnId", "itemId"]
    };
    for key in keys {
        if params[key].as_str().is_none_or(|value| value.is_empty()) {
            return Err(format!("Callback {key} is missing or invalid"));
        }
    }
    if method == "item/permissions/requestApproval" && !params["permissions"].is_object() {
        return Err("Requested permissions are missing".into());
    }
    if let Some(kind) = params
        .get("kind")
        .filter(|_| method == "item/commandExecution/requestApproval")
    {
        if !matches!(kind.as_str(), Some("command" | "stdin" | "humanCommand")) {
            return Err("Unsupported approval kind".into());
        }
    }
    Ok(())
}
