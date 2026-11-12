//! Retain native callback facts and a bounded window of retired RPC identities.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use crate::session::entry_id::EntryId;

/// Preserve the JSON type and spelling of an accepted native RPC identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CallbackKey(pub(in crate::session) String);

impl CallbackKey {
    /// Accept exactly the native string/number identities and bounds used before the split.
    pub(crate) fn new(id: &Value) -> Option<Self> {
        if !(id.is_string() || id.is_number()) || id.as_str().is_some_and(|text| text.len() > 4096)
        {
            return None;
        }
        let key = id.to_string();
        (key.len() <= 4096).then_some(Self(key))
    }

    /// Recover the original typed native identity.
    pub(crate) fn id(&self) -> Value {
        serde_json::from_str(&self.0).expect("typed callback ID")
    }
}

/// Retain callback lifetimes independently of any client's focus or editor.
#[derive(Debug, Default)]
pub(crate) struct CodexCallbacks {
    pub(in crate::session) entries: HashMap<CallbackKey, CodexCallback>,
    recent: VecDeque<CallbackKey>,
    retired: HashSet<CallbackKey>,
}

/// Hold the backend payload and submitted answer while a worker validates it.
#[derive(Debug)]
pub(crate) struct CodexCallback {
    pub(in crate::session) params: Value,
    pub(in crate::session) blocking: bool,
    pub(in crate::session) revision: u64,
    pub(in crate::session) candidate: Option<Value>,
    pub(in crate::session) candidate_card: Option<EntryId>,
    pub(in crate::session) checking: bool,
    pub(in crate::session) interrupting: bool,
    pub(in crate::session) bytes: usize,
}

impl CodexCallbacks {
    /// Detect live duplicates and bounded recent replays from this native process.
    pub(in crate::session) fn contains(&self, id: &Value) -> bool {
        CallbackKey::new(id)
            .is_some_and(|key| self.entries.contains_key(&key) || self.retired.contains(&key))
    }

    /// Retire only this exact typed request, including an unknown but valid identity.
    pub(in crate::session) fn retire(&mut self, id: &Value) -> Option<CallbackKey> {
        let key = CallbackKey::new(id)?;
        self.entries.remove(&key);
        if self.retired.insert(key.clone()) {
            self.recent.push_back(key.clone());
        }
        while self.recent.len() > 2048 {
            if let Some(id) = self.recent.pop_front() {
                self.retired.remove(&id);
            }
        }
        Some(key)
    }

    /// Find a callback using its original typed RPC identity.
    pub(crate) fn get(&self, id: &Value) -> Option<&CodexCallback> {
        self.entries.get(&CallbackKey::new(id)?)
    }

    /// Count callbacks that still await a native answer or resolution.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether this process has any unresolved callback.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Borrow the bounded callback records without copying their payloads.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&CallbackKey, &CodexCallback)> {
        self.entries.iter()
    }

    pub(in crate::session) fn get_mut(&mut self, id: &Value) -> Option<&mut CodexCallback> {
        self.entries.get_mut(&CallbackKey::new(id)?)
    }
}

impl CodexCallback {
    /// Borrow the original callback payload.
    pub(crate) fn params(&self) -> &Value {
        &self.params
    }
    /// Read whether the backend requires an answer before work continues.
    pub(crate) fn blocking(&self) -> bool {
        self.blocking
    }
}

#[cfg(test)]
impl crate::session::state::SessionState {
    /// Seed the submitted worker facts used by existing stale-reply tests.
    pub(crate) fn seed_codex_candidate_for_test(
        &mut self,
        id: &Value,
        revision: u64,
        candidate: Option<Value>,
        card: Option<EntryId>,
        checking: bool,
    ) {
        let entry = self.codex_callbacks.get_mut(id).expect("test callback");
        entry.revision = revision;
        entry.candidate = candidate;
        entry.candidate_card = card;
        entry.checking = checking;
    }
}
