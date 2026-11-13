//! Advances ordered history delivery and sends native interrupt controls.

use std::time::Duration;

use anyhow::Result;
use serde_json::json;

use super::ClaudeClient;
use crate::backend::BackendClient;

impl ClaudeClient {
    /// Let the transport correlate captured interrupt intent with its native reply.
    pub(super) fn interrupt(&self, timeout: Duration) -> Result<String> {
        let commands = self.commands()?;
        let intent = self.prepare_interrupt();
        let result = commands.helper_call("interrupt", json!({}), timeout);
        self.finish_interrupt(intent, result.is_ok());
        result.map(|result| json!({"result":result}).to_string())
    }
}
