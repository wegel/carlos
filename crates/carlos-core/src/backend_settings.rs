//! Describe explicit backend edits and preserve confirmed fields when a later native call fails.

/// Omitted fields stay untouched; a present empty choice restores native inheritance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsPatch {
    /// Change or restore the model only when the user edited it.
    pub model: Option<Option<String>>,
    /// Change or restore effort only when the user edited it.
    pub effort: Option<Option<String>>,
}

/// Distinguish a native refusal from a write whose outcome cannot be established.
#[derive(Debug)]
pub enum SettingsFailure {
    /// The native helper explicitly rejected this field before confirming it.
    Rejected(String),
    /// The helper may have accepted this field, so the caller must not resend it.
    Uncertain(String),
}

/// Keep fields confirmed before another field failed, without claiming atomic native changes.
#[derive(Debug, Default)]
pub struct SettingsResult {
    /// Fields the helper confirmed on this captured connection.
    pub accepted: SettingsPatch,
    /// The first failure; later fields have not been sent.
    pub failure: Option<SettingsFailure>,
}

impl SettingsResult {
    /// Explicitly reject adapters that have no typed settings writer.
    pub fn unsupported() -> Self {
        Self {
            failure: Some(SettingsFailure::Rejected(
                "backend does not support live settings changes".into(),
            )),
            ..Self::default()
        }
    }
}
