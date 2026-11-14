//! Publish compact native queue facts and serve independently addressed preview pages.

use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueueEntry, QueueFacts, QueuePage};
use carlos_proto::session::Backend;
use serde_json::Value;

use super::codex::queue_state::editable_text;
use super::state::SessionState;
use crate::backend::BackendKind;

impl SessionState {
    pub(super) fn publish_queue(&mut self, changes: &mut Vec<super::events::Change>) {
        let fingerprint = self.queue_fingerprint();
        if fingerprint != self.events.queue_fingerprint {
            self.events.queue_fingerprint = fingerprint;
            changes.push(super::events::Change::Queue);
        }
    }

    pub(super) fn export_queue(&self) -> Option<QueueFacts> {
        self.queue_fingerprint()?;
        let state = &self.codex_queue;
        Some(QueueFacts {
            revision: self.queue_revision(),
            known: state.known,
            reading: state.read.is_some(),
            pending: state.operation.is_some(),
            count: state.entries.len() as u32,
            read_error: state.read_error.clone(),
            notice: crate::protocol_events::bounded_diagnostic(&state.notice),
        })
    }

    pub(super) fn queue_revision(&self) -> QueueRevision {
        let mut digest = self.queue_scope_hash();
        digest.update(&self.codex_queue.revision.to_le_bytes());
        digest.update(&[
            u8::from(self.codex_queue.known),
            u8::from(self.codex_queue.exhausted),
        ]);
        digest
            .finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("queue digest")
    }

    pub(super) fn queue_entry_id(&self, entry: &Value) -> QueueEntryId {
        let mut digest = self.queue_scope_hash();
        digest.update(b"entry");
        hash_text(
            &mut digest,
            entry["id"].as_str().expect("accepted queue id"),
        );
        digest
            .finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("queue entry digest")
    }

    pub(super) fn queue_entry(&self, id: &QueueEntryId) -> Option<(usize, &Value)> {
        self.codex_queue
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| self.queue_entry_id(entry) == *id)
    }

    pub(super) fn queue_page(&self, start: usize, limit: usize) -> QueuePage {
        let entries = &self.codex_queue.entries;
        let end = entries
            .len()
            .min(start.saturating_add(limit.clamp(1, carlos_proto::queue::MAX_PAGE_ENTRIES)));
        QueuePage {
            session: self.session_id().clone(),
            seq: self.events.seq,
            queue: self.export_queue().expect("required queue backend"),
            entries: if self.codex_queue.known {
                entries[start..end]
                    .iter()
                    .map(|entry| self.export_queue_entry(entry))
                    .collect()
            } else {
                Vec::new()
            },
            has_more: self.codex_queue.known && end < entries.len(),
        }
    }

    fn export_queue_entry(&self, entry: &Value) -> QueueEntry {
        let (preview, preview_complete) = super::queue_preview::preview(&entry["input"]);
        QueueEntry {
            id: self.queue_entry_id(entry),
            client_message: entry["clientUserMessageId"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(128)
                .collect(),
            preview,
            preview_complete,
            text_bytes: editable_text(entry)
                .map(|text| carlos_proto::bytes::ByteLength::new(text.len() as u64)),
        }
    }

    fn queue_fingerprint(&self) -> Option<blake3::Hash> {
        if self.events.backend != Backend::Codex || !self.codex_scope().enabled() {
            return None;
        }
        let state = &self.codex_queue;
        let mut digest = blake3::Hasher::new();
        digest.update(self.queue_revision().as_str().as_bytes());
        digest.update(&[
            u8::from(state.read.is_some()),
            u8::from(state.operation.is_some()),
        ]);
        digest.update(&(state.entries.len() as u64).to_le_bytes());
        hash_text(&mut digest, state.read_error.as_deref().unwrap_or_default());
        hash_text(&mut digest, &state.notice);
        Some(digest.finalize())
    }

    fn queue_scope_hash(&self) -> blake3::Hasher {
        let mut digest = blake3::Hasher::new();
        digest.update(b"carlos.queue.v1");
        hash_text(&mut digest, self.session_id().as_str());
        hash_text(&mut digest, self.thread_id());
        let (root, helper) = self.command_connection(BackendKind::Codex);
        digest.update(&root.to_le_bytes());
        digest.update(&helper.to_le_bytes());
        digest.update(&[u8::from(self.codex_scope().closed())]);
        digest
    }
}

fn hash_text(digest: &mut blake3::Hasher, text: &str) {
    digest.update(&(text.len() as u64).to_le_bytes());
    digest.update(text.as_bytes());
}
