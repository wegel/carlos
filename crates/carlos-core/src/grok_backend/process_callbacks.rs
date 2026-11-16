//! Retain original ACP reply targets while owner cards carry private transport tokens.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use crate::dispatch_guard::DispatchRetired;

const MAX_CALLBACKS: usize = 256;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Callbacks {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    closed: bool,
    next: u64,
    tokens: HashMap<String, Arc<Callback>>,
    ids: HashMap<String, String>,
    bytes: usize,
}

pub(super) struct Callback {
    target: Target,
    charge: usize,
    live: AtomicBool,
    dispatched: AtomicBool,
    cancelling: AtomicBool,
}

struct Target {
    token: String,
    id: Value,
    session: Option<String>,
    epoch: u64,
    fingerprint: blake3::Hash,
}

pub(super) struct Registered {
    pub token: String,
    pub retired: Option<String>,
}

impl Callbacks {
    pub(super) fn register(
        &self,
        request: &Value,
        charge: usize,
        epoch: u64,
    ) -> Result<Registered> {
        let id = &request["id"];
        ensure!(
            id.is_string() || id.as_i64().is_some() || id.as_u64().is_some(),
            "invalid Grok callback id"
        );
        let fingerprint = blake3::hash(&serde_json::to_vec(request)?);
        let key = id.to_string();
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Grok callbacks lock poisoned"))?;
        ensure!(!state.closed, "Grok callbacks are closed");
        let previous = state.ids.get(&key).cloned();
        if let Some(old) = previous.as_ref().and_then(|token| state.tokens.get(token)) {
            if old.target.fingerprint == fingerprint && old.target.epoch == epoch {
                return Ok(Registered {
                    token: old.token().to_owned(),
                    retired: None,
                });
            }
            ensure!(
                !old.dispatched.load(Ordering::SeqCst),
                "Grok reused a callback id during its reply"
            );
        }
        if let Some(previous) = &previous {
            state.retire(previous);
        }
        ensure!(
            state.tokens.len() < MAX_CALLBACKS && charge <= MAX_BYTES.saturating_sub(state.bytes),
            "Grok callback backlog exceeded its count or byte budget"
        );
        state.next = state
            .next
            .checked_add(1)
            .context("Grok callback tokens exhausted")?;
        let token = format!("grok-request-{}", state.next);
        let target = Target {
            token: token.clone(),
            id: id.clone(),
            session: request["params"]["sessionId"].as_str().map(str::to_owned),
            fingerprint,
            epoch,
        };
        let callback = Arc::new(Callback::new(target, charge));
        state.bytes += charge;
        state.ids.insert(key, token.clone());
        state.tokens.insert(token.clone(), callback);
        Ok(Registered {
            token,
            retired: previous,
        })
    }

    pub(super) fn get(&self, token: &Value) -> Result<Arc<Callback>> {
        let token = token.as_str().ok_or(DispatchRetired)?;
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Grok callbacks lock poisoned"))?
            .tokens
            .get(token)
            .cloned()
            .ok_or_else(|| DispatchRetired.into())
    }

    pub(super) fn dispatch(
        &self,
        callback: &Arc<Callback>,
        cancellation: bool,
    ) -> Result<(), DispatchRetired> {
        let state = self.state.lock().map_err(|_| DispatchRetired)?;
        let current = state.tokens.get(callback.token()).ok_or(DispatchRetired)?;
        if !Arc::ptr_eq(current, callback) {
            return Err(DispatchRetired);
        }
        callback.dispatch(cancellation)
    }

    pub(super) fn finish(&self, token: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.retire(token);
        }
    }

    pub(super) fn cancel_session(
        &self,
        session: Option<&str>,
        epoch: Option<u64>,
    ) -> Vec<Arc<Callback>> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        state
            .tokens
            .values()
            .filter(|callback| {
                session.is_none_or(|session| callback.session() == Some(session))
                    && epoch.is_none_or(|epoch| callback.epoch() == epoch)
                    && !callback.dispatched.load(Ordering::SeqCst)
            })
            .map(|callback| {
                callback.cancelling.store(true, Ordering::SeqCst);
                callback.clone()
            })
            .collect()
    }

    pub(super) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            for callback in state.tokens.values() {
                callback.live.store(false, Ordering::SeqCst);
            }
            state.tokens.clear();
            state.ids.clear();
            state.bytes = 0;
        }
    }
}

impl Callback {
    fn new(target: Target, charge: usize) -> Self {
        Self {
            target,
            charge,
            live: AtomicBool::new(true),
            dispatched: AtomicBool::new(false),
            cancelling: AtomicBool::new(false),
        }
    }

    pub(super) fn token(&self) -> &str {
        &self.target.token
    }
    pub(super) fn id(&self) -> &Value {
        &self.target.id
    }
    pub(super) fn session(&self) -> Option<&str> {
        self.target.session.as_deref()
    }

    pub(super) fn epoch(&self) -> u64 {
        self.target.epoch
    }

    /// Only the writer calls this; enqueue checks must never consume a callback.
    fn dispatch(&self, cancellation: bool) -> Result<(), DispatchRetired> {
        if !self.live.load(Ordering::SeqCst)
            || !cancellation && self.cancelling.load(Ordering::SeqCst)
            || self
                .dispatched
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return Err(DispatchRetired);
        }
        Ok(())
    }
}

impl State {
    fn retire(&mut self, token: &str) {
        let Some(callback) = self.tokens.remove(token) else {
            return;
        };
        callback.live.store(false, Ordering::SeqCst);
        self.bytes -= callback.charge;
        let key = callback.id().to_string();
        if self.ids.get(&key).is_some_and(|current| current == token) {
            self.ids.remove(&key);
        }
    }
}

#[cfg(test)]
#[path = "process_callback_tests.rs"]
mod tests;
