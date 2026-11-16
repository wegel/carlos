//! Correlate Grok ACP requests independently from process ownership and event parsing.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use super::process_client::{cancel_agent_requests, retire_cards};
use super::GrokIo;
use crate::dispatch_guard::DispatchGuard;

impl GrokIo {
    pub(in crate::grok_backend) fn respond_guarded(
        &self,
        token: &Value,
        body: Value,
        error: bool,
        permission: DispatchGuard,
        scope: impl Fn(Option<&str>, u64) -> bool + Send + Sync + 'static,
    ) -> Result<()> {
        let callback = self.stdin.callbacks().get(token)?;
        let target = callback.clone();
        let permission = permission.and(DispatchGuard::new(move || {
            scope(target.session(), target.epoch())
        }));
        self.stdin.write_callback(callback, body, error, permission)
    }

    pub(in crate::grok_backend) fn retire_callback_choices(
        &self,
        session: &str,
        epoch: u64,
    ) -> Result<()> {
        let tokens = self
            .stdin
            .callbacks()
            .cancel_session(Some(session), Some(epoch))
            .iter()
            .map(|callback| callback.token().to_owned())
            .collect();
        // Retire the owner cards before a failing or uncertain cancel write can close the table.
        retire_cards(&self.events, tokens)
    }

    pub(in crate::grok_backend) fn cancel_callbacks(
        &self,
        session: &str,
        epoch: Option<u64>,
    ) -> Result<()> {
        cancel_agent_requests(&self.stdin, &self.events, Some(session), epoch)
    }

    pub(in crate::grok_backend) fn cancel(&self) {
        self.events.close();
        self.stdin.close();
    }

    pub(in crate::grok_backend) fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<String> {
        self.request_guarded(method, params, timeout, DispatchGuard::default())
    }

    pub(in crate::grok_backend) fn request_guarded(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        permission: DispatchGuard,
    ) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("Grok pending lock poisoned"))?
            .insert(id, tx);
        if let Err(error) = self.send_json(
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            permission,
        ) {
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

    pub(in crate::grok_backend) fn notify_guarded(
        &self,
        method: &str,
        params: Value,
        permission: DispatchGuard,
    ) -> Result<()> {
        self.send_json(
            json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            }),
            permission,
        )
    }

    pub(in crate::grok_backend) fn start_prompt(
        &self,
        params: Value,
        permission: DispatchGuard,
    ) -> Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.prompt_in_flight
            .lock()
            .map_err(|_| anyhow!("Grok prompt lock poisoned"))?
            .insert(id);
        if let Err(error) = self.send_json(
            json!({"jsonrpc":"2.0","id":id,"method":"session/prompt","params":params}),
            permission,
        ) {
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

    fn send_json(&self, value: Value, permission: DispatchGuard) -> Result<()> {
        self.stdin.write_guarded(&value, permission)
    }
}
