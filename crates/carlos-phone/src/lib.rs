//! Expose the shared checked client to Swift while the phone owns its socket and UI.

mod changes;
mod client;
mod entries;
mod error;
mod operations;
mod records;
mod session_calls;
mod sessions;
mod transcript;

pub use changes::{PhoneSessionChange, PhoneWindowChange};
pub use client::PhoneClient;
pub use entries::{
    PhoneDiff, PhoneEntry, PhoneEntryFormat, PhoneEntryKind, PhoneLabelSource, PhoneText,
    PhoneTool, PhoneToolKind, PhoneToolStatus,
};
pub use error::PhoneError;
pub use operations::{PhoneOperation, PhoneOperationState, PhoneSubmission};
pub use records::{PhoneBatch, PhoneFailure, PhoneFrame};
pub use sessions::{
    PhoneActivity, PhoneAvailability, PhoneBackend, PhoneContext, PhonePhase, PhoneQuota,
    PhoneSession, PhoneUsage,
};
pub use transcript::PhoneTranscript;

uniffi::setup_scaffolding!();

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod session_call_tests;
