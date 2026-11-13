//! Validated opaque addresses and lossless event counters.

use std::fmt;
use std::str::FromStr;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// An invalid opaque id or event counter supplied by a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidId;

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid canonical id or sequence")
    }
}

impl std::error::Error for InvalidId {}

macro_rules! opaque_id {
    ($name:ident, $length:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Borrow the canonical opaque address.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidId;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                let valid = value.len() == $length
                    && value.bytes().enumerate().all(|(i, b)| {
                        if $length == 49 && i == 32 {
                            b == b'-'
                        } else {
                            b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                        }
                    });
                if valid {
                    Ok(Self(value))
                } else {
                    Err(InvalidId)
                }
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                value.to_owned().try_into()
            }
        }
    };
}

opaque_id!(
    SessionId,
    32,
    "An opaque session address independent of native conversation ids."
);
opaque_id!(
    EntryId,
    49,
    "An opaque row or approval address, never a vector offset."
);

opaque_id!(
    ClientId,
    32,
    "A client identity bound to an authenticated daemon connection."
);
opaque_id!(
    OperationId,
    32,
    "An idempotency and outcome key for one submitted client command."
);

/// A per-session cursor encoded as decimal text to avoid JavaScript integer rounding.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sequence(u64);

impl Sequence {
    /// Construct a cursor, with zero denoting the state before the first event.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Read the cursor without narrowing it.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advance without wrapping or reusing a published cursor.
    pub fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl Serialize for Sequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Sequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(de::Error::custom(InvalidId));
        }
        value.parse::<u64>().map(Self).map_err(de::Error::custom)
    }
}
