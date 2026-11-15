//! Retain bounded captured native windows and the original reader that may use each one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use carlos_proto::ids::{TaskRevision, TaskWindowId};
use serde_json::Value;

use super::codex::panel_state::{Process, Request, Snapshot, VerifiedOwners};
use super::state::SessionState;

/// A socket/session interest owns this flag; dropping it retires every associated native window.
pub(in crate::session) type Reader = Arc<AtomicBool>;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::session) struct Scope {
    digest: blake3::Hash,
    epoch: u64,
    generation: u64,
}

impl Scope {
    pub fn capture(session: &SessionState) -> Self {
        Self {
            digest: session.task_scope().finalize(),
            epoch: session.codex_scope().epoch(),
            generation: session.codex_connection().generation(),
        }
    }
}

#[derive(Debug)]
pub(in crate::session) struct Captured {
    pub agents: Vec<Value>,
    pub processes: Vec<Process>,
    pub owners: VerifiedOwners,
    pub process_scopes: HashMap<String, Option<String>>,
    pub reports: HashMap<String, Value>,
    pub older: Value,
    pub bytes: usize,
    pub revision: TaskRevision,
    pub read_error: Option<String>,
}

impl Captured {
    pub fn rows(&self) -> usize {
        self.agents.len() + self.processes.len()
    }
}

#[derive(Debug)]
pub(in crate::session) enum Contents {
    Reading(Snapshot),
    Ready(Captured),
    Failed(String),
    Retired { request: Request, started: Instant },
}

#[derive(Debug)]
pub(in crate::session) struct Window {
    pub reader: Reader,
    pub scope: Scope,
    pub contents: Contents,
}

impl Window {
    pub fn available(&self, session: &SessionState, reader: &Reader) -> bool {
        self.reader.load(Ordering::Acquire)
            && Arc::ptr_eq(&self.reader, reader)
            && self.scope == Scope::capture(session)
    }

    pub fn bytes(&self) -> usize {
        match &self.contents {
            Contents::Reading(read) => read.bytes,
            Contents::Ready(data) => data.bytes,
            Contents::Failed(_) | Contents::Retired { .. } => 0,
        }
    }
}

/// Latest and all older captures share one native metadata allowance, never one per client.
#[derive(Debug, Default)]
pub(in crate::session) struct TaskWindows {
    pub slots: HashMap<TaskWindowId, Window>,
    pub latest_bytes: Option<usize>,
}

impl TaskWindows {
    pub fn started(&self, token: u64) -> Option<Instant> {
        self.slots.values().find_map(|slot| match &slot.contents {
            Contents::Reading(read) if read.request.token == token => Some(read.started),
            _ => None,
        })
    }

    pub fn wait(&self, now: Instant) -> Option<std::time::Duration> {
        self.slots
            .values()
            .filter_map(|slot| match &slot.contents {
                Contents::Reading(read) => Some(read.started),
                Contents::Retired { started, .. } => Some(*started),
                _ => None,
            })
            .map(|started| {
                (started + std::time::Duration::from_secs(20)).saturating_duration_since(now)
            })
            .min()
    }
    pub fn bytes(&self) -> usize {
        self.slots.values().map(Window::bytes).sum()
    }

    pub fn request(&self, token: u64) -> Option<(&TaskWindowId, &Request)> {
        self.slots.iter().find_map(|(id, slot)| {
            let request = match &slot.contents {
                Contents::Reading(read) => &read.request,
                Contents::Retired { request, .. } => request,
                _ => return None,
            };
            (request.token == token).then_some((id, request))
        })
    }
}
