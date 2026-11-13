//! Bind known native echo tokens to their submitting operation without comparing prompt text.

use std::collections::{HashMap, VecDeque};

use carlos_proto::submissions::ClientOperation;
use serde_json::Value;
use uuid::Uuid;

use super::entry_id::EntryId;
use super::state::SessionState;
use super::transcript_journal::RowChange;

const MAX_PENDING: usize = 1024;

#[derive(Debug, Default)]
pub(super) struct OperationEchoes {
    pending: VecDeque<(String, ClientOperation)>,
    rows: HashMap<EntryId, ClientOperation>,
}

impl SessionState {
    pub(super) fn operation_capacity(&self, operation: Option<&ClientOperation>) -> bool {
        operation.is_none() || self.events.operations.pending.len() < MAX_PENDING
    }

    pub(super) fn prepare_operation_token(
        &mut self,
        params: &mut Value,
        operation: Option<&ClientOperation>,
    ) {
        let Some(operation) = operation else {
            return;
        };
        let token = Uuid::new_v4().to_string();
        params["clientUserMessageId"] = Value::String(token.clone());
        self.events
            .operations
            .pending
            .push_back((token, operation.clone()));
    }

    pub(super) fn bind_native_operation(&mut self, item: &Value) {
        let Some(native) = item["id"].as_str() else {
            return;
        };
        let Some(token) = item["clientId"]
            .as_str()
            .or_else(|| native.strip_prefix("claude-prompt:"))
        else {
            return;
        };
        let Some(index) = self.item_message_index(&self.backend_entry_key(native)) else {
            return;
        };
        let Some(position) = self
            .events
            .operations
            .pending
            .iter()
            .position(|(known, _)| known == token)
        else {
            return;
        };
        let (_, operation) = self
            .events
            .operations
            .pending
            .remove(position)
            .expect("known token");
        self.bind_entry_operation(index, Some(&operation));
    }

    pub(super) fn bind_entry_operation(
        &mut self,
        index: usize,
        operation: Option<&ClientOperation>,
    ) {
        let Some(operation) = operation else {
            return;
        };
        let id = self.messages()[index].id;
        if self.events.operations.rows.get(&id) == Some(operation) {
            return;
        }
        self.events.operations.rows.insert(id, operation.clone());
        self.messages.journal.record(RowChange {
            id,
            index,
            replace: true,
            delta: None,
            facts: false,
        });
    }

    pub(super) fn entry_operation(&self, id: EntryId) -> Option<ClientOperation> {
        self.events.operations.rows.get(&id).cloned()
    }

    pub(super) fn retain_entry_operations(&mut self) {
        self.events
            .operations
            .rows
            .retain(|id, _| self.messages.entry(*id).is_some());
    }
}
