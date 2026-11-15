//! Keep ordered approval cards and bound their retained encoded facts before accepting updates.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use carlos_proto::ids::EntryId;
use carlos_proto::session::Approval;

use crate::ClientError;

const MAX_CARDS: usize = 4096;
const MAX_CARD_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct Cards {
    entries: Vec<Approval>,
    weights: HashMap<EntryId, usize>,
    bytes: usize,
}

impl Cards {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytes
    }
    pub fn new(entries: Vec<Approval>) -> Result<Self, ClientError> {
        let mut cards = Self {
            entries: Vec::new(),
            weights: HashMap::new(),
            bytes: 0,
        };
        let mut ids = HashSet::new();
        for entry in entries {
            if !ids.insert(entry.id.clone()) {
                return Err(ClientError::DuplicateEntry(entry.id));
            }
            cards.insert(entry, None)?;
        }
        Ok(cards)
    }

    pub fn entries(&self) -> &[Approval] {
        &self.entries
    }

    pub fn contains(&self, id: &EntryId) -> bool {
        self.weights.contains_key(id)
    }

    pub fn remove(&mut self, id: &EntryId) {
        self.bytes -= self.weights.remove(id).unwrap_or(0);
        self.entries.retain(|card| &card.id != id);
    }

    pub fn insert(&mut self, card: Approval, before: Option<EntryId>) -> Result<(), ClientError> {
        if card
            .caller
            .as_ref()
            .is_some_and(|caller| caller.len() > 4096)
        {
            return Err(ClientError::InvalidPayload(
                "approval caller exceeds its display byte bound".into(),
            ));
        }
        if before.as_ref() == Some(&card.id) || before.as_ref().is_some_and(|id| !self.contains(id))
        {
            return Err(ClientError::InvalidPayload(
                "approval order has a missing or self anchor".into(),
            ));
        }
        let mut count = CountBytes(0);
        serde_json::to_writer(&mut count, &card)
            .map_err(|_| ClientError::Capacity("approval card exceeds eight MiB"))?;
        let total = self.bytes - self.weights.get(&card.id).copied().unwrap_or(0) + count.0;
        if total > MAX_CARD_BYTES || (!self.contains(&card.id) && self.entries.len() >= MAX_CARDS) {
            return Err(ClientError::Capacity("client approval facts are full"));
        }
        self.remove(&card.id);
        let index = before
            .and_then(|id| self.entries.iter().position(|card| card.id == id))
            .unwrap_or(self.entries.len());
        self.bytes = total;
        self.weights.insert(card.id.clone(), count.0);
        self.entries.insert(index, card);
        Ok(())
    }
}

struct CountBytes(usize);

impl Write for CountBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_CARD_BYTES.saturating_sub(self.0) {
            return Err(io::Error::other("approval byte limit"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
