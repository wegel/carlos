//! Advances ordered history delivery and sends native interrupt controls.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::ClaudeClient;

impl ClaudeClient {
    /// Advance decoded history through the same batch gate as both native readers.
    pub(super) fn continue_history(&self, params: &Value) -> Result<()> {
        let transport = self
            .transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .clone();
        if params["generation"]
            .as_u64()
            .is_some_and(|generation| generation != transport.generation)
        {
            return Ok(());
        }
        transport.emitter.continue_history(params["epoch"].as_u64())
    }

    /// Let the transport correlate captured interrupt intent with its native reply.
    pub(super) fn interrupt(&self, timeout: Duration) -> Result<String> {
        let transport = self
            .transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .clone();
        let ids = self.handler.borrow_mut().lifecycle.begin_interrupt();
        let result = match transport.call("interrupt", json!({}), timeout) {
            Ok(result) => result,
            Err(error) => {
                self.handler.borrow_mut().lifecycle.rollback_interrupt(&ids);
                return Err(error);
            }
        };
        Ok(json!({"result":result}).to_string())
    }
}
