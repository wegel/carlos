//! Correlate Grok ACP requests independently from process ownership and event parsing.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use super::GrokIo;

impl GrokIo {
    pub(in crate::grok_backend) fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("Grok pending lock poisoned"))?
            .insert(id, tx);
        if let Err(error) =
            self.send_json(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
        {
            self.pending.lock().expect("Grok pending").remove(&id);
            return Err(error);
        }
        match rx.recv_timeout(timeout) {
            Ok(response) => Ok(response),
            Err(_) => {
                let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
                bail!("timeout waiting for Grok {method}");
            }
        }
    }

    pub(in crate::grok_backend) fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.send_json(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    pub(in crate::grok_backend) fn start_prompt(&self, params: Value) -> Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.prompt_in_flight
            .lock()
            .map_err(|_| anyhow!("Grok prompt lock poisoned"))?
            .insert(id);
        if let Err(error) = self
            .send_json(json!({"jsonrpc":"2.0","id":id,"method":"session/prompt","params":params}))
        {
            self.prompt_in_flight
                .lock()
                .expect("Grok prompts")
                .remove(&id);
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::grok_backend) fn clear_in_flight_prompts(&self) {
        let _ = self
            .prompt_in_flight
            .lock()
            .map(|mut prompts| prompts.clear());
    }

    fn send_json(&self, value: Value) -> Result<()> {
        self.stdin.write(&value)
    }
}
