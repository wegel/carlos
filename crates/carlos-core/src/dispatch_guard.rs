//! Carry a command's permission to the native writer that sends its bytes.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A worker and its queued native frames share the same retirement check.
#[derive(Clone, Default)]
pub struct DispatchGuard(Option<Arc<dyn Fn() -> bool + Send + Sync>>);

/// A retired command wrote no bytes from its queued frame.
#[derive(Debug)]
pub struct DispatchRetired;

impl DispatchGuard {
    /// Keep the session owner's turn, card or ordinary-command permission.
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self::new(move || flag.load(Ordering::SeqCst))
    }

    /// Check captured native identity immediately before a frame starts writing.
    pub fn new(check: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(check)))
    }

    /// Require both the owner's permission and the captured native identity.
    pub fn and(self, other: Self) -> Self {
        match (self.0, other.0) {
            (None, right) => Self(right),
            (left, None) => Self(left),
            (Some(left), Some(right)) => Self::new(move || left() && right()),
        }
    }

    /// Return a typed pre-write failure without closing the healthy native transport.
    pub fn check(&self) -> Result<(), DispatchRetired> {
        if self.0.as_ref().is_none_or(|check| check()) {
            Ok(())
        } else {
            Err(DispatchRetired)
        }
    }
}

impl fmt::Display for DispatchRetired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("command retired before native writer dispatch; no bytes sent")
    }
}
impl std::error::Error for DispatchRetired {}
