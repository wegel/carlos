//! Keep source lengths and offsets distinct while preserving all bits in JSON.

use serde::{Deserialize, Serialize};

use crate::ids::Sequence;

/// A byte position in an original body, encoded as canonical decimal text.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteOffset(Sequence);

impl ByteOffset {
    /// Construct an exact byte position without narrowing it to a platform's usize.
    pub const fn new(value: u64) -> Self {
        Self(Sequence::new(value))
    }

    /// Read the position for checked platform indexing.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// A body length, distinct from both an entry position and a session event cursor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteLength(Sequence);

impl ByteLength {
    /// Construct an exact source length.
    pub const fn new(value: u64) -> Self {
        Self(Sequence::new(value))
    }

    /// Read the length for a bounded cache or progress indicator.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
