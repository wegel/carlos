//! Correlate synchronous requests and guarded answers on one native Codex transport.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::{history_reply, timeout_closes_connection, timeout_message, AppServerClient};
use crate::dispatch_guard::DispatchGuard;
use crate::protocol_state::{request_key, Target};

impl AppServerClient {
    /// Include enqueueing, pipe writes and the response in one request deadline.
    pub fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.call_guarded(method, params, timeout, DispatchGuard::default())
    }

    pub(crate) fn call_guarded(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        permission: DispatchGuard,
    ) -> Result<String> {
        let deadline = Instant::now() + timeout;
        let id = self.state.next_id();
        let key = request_key(&id).expect("native client ID");
        let (reply, receiver) = mpsc::channel();
        let target = if history_reply(method, &params) {
            Target::History(reply)
        } else {
            Target::Caller(reply)
        };
        self.state.insert(key.clone(), target)?;
        if let Err(error) = self.writer.write_guarded(
            json!({"id":id,"method":method,"params":params}),
            deadline,
            &self.state,
            permission,
        ) {
            self.state.take(&key);
            self.state.finish_large_reply(&key);
            return Err(error);
        }
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => bail!(error),
            Err(_) => {
                self.state.take(&key);
                let error = timeout_message(method);
                if timeout_closes_connection(method) {
                    self.state.close(
                        &format!("{error}; request may already have been accepted"),
                        true,
                    );
                }
                bail!(error);
            }
        }
    }

    pub(crate) fn respond_guarded(
        &self,
        request_id: &Value,
        result: Value,
        permission: DispatchGuard,
    ) -> Result<()> {
        self.writer.write_guarded(
            json!({"id":request_id,"result":result}),
            Instant::now() + Duration::from_secs(2),
            &self.state,
            permission,
        )
    }
}
