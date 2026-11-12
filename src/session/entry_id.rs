//! Carlos entry ids, allocated once per inserted entry from a private namespace.

use uuid::Uuid;

/// Identifies one Carlos entry independently of its backend key or vector position.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EntryId {
    namespace: Uuid,
    counter: u64,
}

pub(super) struct EntryIds {
    namespace: Uuid,
    next: u64,
}

impl EntryIds {
    pub(super) fn new() -> Self {
        Self {
            namespace: Uuid::new_v4(),
            next: 0,
        }
    }

    pub(super) fn allocate(&mut self) -> EntryId {
        let counter = self.next;
        self.next = counter.checked_add(1).expect("entry id counter exhausted");
        EntryId {
            namespace: self.namespace,
            counter,
        }
    }
}

#[cfg(test)]
#[path = "entry_id_tests.rs"]
mod tests;
