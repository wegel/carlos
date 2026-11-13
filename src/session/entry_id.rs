//! Carlos entry ids, allocated once per inserted entry from a private namespace.

use uuid::Uuid;

/// Identifies one Carlos entry independently of its backend key or vector position.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EntryId {
    namespace: Uuid,
    counter: u64,
}

#[cfg_attr(test, derive(Debug))]
pub(super) struct EntryIds {
    namespace: Uuid,
    next: u64,
}

impl Default for EntryIds {
    fn default() -> Self {
        Self::new()
    }
}

impl EntryId {
    /// Compare counters only after checking that both ids belong to the same namespace.
    pub(super) fn offset_from(self, first: Self) -> Option<usize> {
        if self.namespace != first.namespace {
            return None;
        }
        usize::try_from(self.counter.checked_sub(first.counter)?).ok()
    }
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

#[cfg(test)]
impl EntryId {
    /// Keep the allocator namespace but choose a counter far beyond this test's live range.
    pub(crate) fn unallocated_for_approval_test(self) -> Self {
        Self {
            counter: self.counter + 1_000_000,
            ..self
        }
    }

    /// Build arbitrary stale, foreign, and never-issued ids for adversarial tests.
    pub(crate) fn for_approval_test(namespace: Uuid, counter: u64) -> Self {
        Self { namespace, counter }
    }
}
