//! Expose the shared checked client to Swift while the phone owns its socket and UI.

mod client;
mod error;
mod records;

pub use client::PhoneClient;
pub use error::PhoneError;
pub use records::{PhoneBatch, PhoneFailure, PhoneFrame};

uniffi::setup_scaffolding!();
