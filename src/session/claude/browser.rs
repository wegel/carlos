//! Track native browser consent and completion independently of answer forms.

use serde_json::Value;

use crate::session::state::SessionState;

/// Keep callback identity until consent and native completion have both arrived.
#[derive(Debug)]
pub(in crate::session) struct BrowserRequest {
    callback_id: String,
    consented: bool,
    completed: bool,
}

/// Tell the client which draft still needs consent after a first completion.
pub(crate) struct BrowserCompletion {
    pub(crate) server: String,
    pub(crate) callback_id: Option<String>,
}

impl SessionState {
    /// Remember a browser request when its callback is accepted by the app.
    pub(crate) fn register_claude_browser(&mut self, id: &str, payload: &Value) {
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

    /// Record consent after a successful answer, returning the server and prior completion.
    pub(crate) fn answer_claude_browser(
        &mut self,
        payload: &Value,
        accepted: bool,
    ) -> Option<(String, bool)> {
        let key = browser_key(payload)?;
        if !accepted {
            self.claude.browser.remove(&key);
            return None;
        }
        let completed = self.claude.browser.get_mut(&key).is_some_and(|request| {
            request.consented = true;
            request.completed
        });
        if completed {
            self.claude.browser.remove(&key);
        }
        Some((key.0, completed))
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
