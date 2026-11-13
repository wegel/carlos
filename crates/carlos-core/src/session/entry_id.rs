//! Carlos entry ids, allocated once per inserted entry from a private namespace.

use uuid::Uuid;

/// Identifies one Carlos entry independently of its backend key or vector position.
///
/// A client cannot rewrite an issued id's counter:
///
/// ```compile_fail
/// use carlos_core::session::entry_id::EntryId;
/// fn overwrite(id: &mut EntryId) {
///     id.counter = 0;
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EntryId {
    namespace: Uuid,
    counter: u64,
}

#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
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
    /// Export an opaque address without exposing native ids or row positions.
    pub fn to_wire(self) -> carlos_proto::ids::EntryId {
        format!("{}-{:016x}", self.namespace.simple(), self.counter)
            .parse()
            .expect("core emits canonical ids")
    }

    /// Decode an address for lookup; parsing does not make an unissued id live.
    pub fn from_wire(id: &carlos_proto::ids::EntryId) -> Self {
        let (namespace, counter) = id.as_str().split_once('-').expect("validated id");
        Self {
            namespace: Uuid::parse_str(namespace).expect("validated namespace"),
            counter: u64::from_str_radix(counter, 16).expect("validated counter"),
        }
    }

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

#[cfg(any(test, feature = "test-support"))]
impl EntryId {
    /// Keep the allocator namespace but choose a counter far beyond this test's live range.
    pub fn unallocated_for_approval_test(self) -> Self {
        Self {
            counter: self.counter + 1_000_000,
            ..self
        }
    }

    /// Build arbitrary stale, foreign, and never-issued ids for adversarial tests.
    pub fn for_approval_test(namespace: Uuid, counter: u64) -> Self {
        Self { namespace, counter }
    }
}
