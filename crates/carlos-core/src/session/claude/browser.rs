//! Track native browser consent and completion independently of answer forms.

use std::collections::HashMap;

use serde_json::Value;

use crate::session::approval_state::ApprovalChoice;
#[cfg(any(test, feature = "test-support"))]
use crate::session::approval_state::ApprovalRequestDraft;
use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::text::safe_label;

/// Keep callback identity until consent and native completion have both arrived.
#[derive(Debug)]
pub(in crate::session) struct BrowserRequest {
    callback_id: String,
    consented: bool,
    completed: bool,
}

/// Tell the client which draft still needs consent after a first completion.
pub(crate) struct BrowserCompletion {
    pub server: String,
    pub callback_id: Option<String>,
}

impl SessionState {
    /// Remember a browser request when its callback is accepted by the app.
    pub fn register_claude_browser(&mut self, id: &str, payload: &Value) {
        let Some(key) = browser_key(payload) else {
            return;
        };
        self.claude.browser.insert(
            key,
            BrowserRequest {
                callback_id: id.to_string(),
                consented: false,
                completed: false,
            },
        );
    }

    /// Withdraw only requests owned by the cancelled callback.
    pub(crate) fn withdraw_claude_browser(&mut self, id: &str) {
        self.claude
            .browser
            .retain(|_, request| request.callback_id != id);
    }

    /// Accept one completion, retaining early completion until the user answers.
    pub(crate) fn complete_claude_browser(&mut self, message: &Value) -> Option<BrowserCompletion> {
        let (Some(server), Some(id)) = (
            message["mcp_server_name"].as_str(),
            message["elicitation_id"].as_str(),
        ) else {
            return None;
        };
        let key = (server.to_string(), id.to_string());
        let request = self.claude.browser.get_mut(&key)?;
        if request.completed {
            return None;
        }
        request.completed = true;
        let callback_id = if request.consented {
            self.claude.browser.remove(&key);
            None
        } else {
            Some(request.callback_id.clone())
        };
        Some(BrowserCompletion {
            server: key.0,
            callback_id,
        })
    }
}

fn browser_key(payload: &Value) -> Option<(String, String)> {
    if payload["mode"] != "url" {
        return None;
    }
    Some((
        payload["serverName"].as_str()?.to_string(),
        payload["elicitationId"].as_str()?.to_string(),
    ))
}

impl SessionState {
    /// Append the original consent notice only after the backend accepted the answer.
    #[cfg(any(test, feature = "test-support"))]
    pub fn finish_claude_browser_answer(
        &mut self,
        pending: &ApprovalRequestDraft,
        choice: ApprovalChoice,
    ) {
        self.append_browser_consent(pending.requested_permissions.as_ref(), choice)
    }

    /// Read the active card without cloning its potentially large native payload.
    pub(in crate::session) fn finish_pending_claude_browser_answer(
        &mut self,
        choice: ApprovalChoice,
    ) {
        let payload = self
            .approval
            .pending()
            .and_then(|p| p.facts().requested_permissions.as_ref());
        let notice = browser_consent_notice(&mut self.claude.browser, payload, choice);
        self.append_browser_notice(notice)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn append_browser_consent(&mut self, payload: Option<&Value>, choice: ApprovalChoice) {
        let notice = browser_consent_notice(&mut self.claude.browser, payload, choice);
        self.append_browser_notice(notice)
    }

    fn append_browser_notice(&mut self, notice: Option<String>) {
        if let Some(notice) = notice {
            self.append_message(Role::System, notice);
        }
    }

    /// Append a once-only completion and identify any editor still waiting for consent.
    pub fn receive_claude_browser_completion(&mut self, message: &Value) -> Option<Option<String>> {
        let completion = self.complete_claude_browser(message)?;
        let _ = self.append_message(
            Role::System,
            format!(
                "Browser interaction completed for {}",
                safe_label(&completion.server)
            ),
        );
        Some(completion.callback_id)
    }
}

fn answer_browser(
    browser: &mut HashMap<(String, String), BrowserRequest>,
    payload: &Value,
    accepted: bool,
) -> Option<(String, bool)> {
    let key = browser_key(payload)?;
    if !accepted {
        browser.remove(&key);
        return None;
    }
    let completed = browser.get_mut(&key).is_some_and(|request| {
        request.consented = true;
        request.completed
    });
    if completed {
        browser.remove(&key);
    }
    Some((key.0, completed))
}

fn browser_consent_notice(
    browser: &mut HashMap<(String, String), BrowserRequest>,
    payload: Option<&Value>,
    choice: ApprovalChoice,
) -> Option<String> {
    let payload = payload?;
    let (server, completed) = answer_browser(browser, payload, choice == ApprovalChoice::Accept)?;
    let state = if completed {
        "The server already reported completion."
    } else {
        "Complete the interaction in your browser; the server has not reported completion."
    };
    Some(format!(
        "Browser consent sent to {}. Link: {}. {state}",
        safe_label(&server),
        safe_label(payload["url"].as_str().unwrap_or("not supplied"))
    ))
}
