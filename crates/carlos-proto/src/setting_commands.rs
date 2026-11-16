//! Share explicit settings write payloads while the daemon keeps session routing outside the owner.

use serde::{Deserialize, Serialize};

use crate::context_defaults::ContextChoice;
use crate::ids::SettingsRevision;
use crate::settings::SettingsChange;
use crate::submissions::ClientOperation;

/// Change one host default after the daemon selects its owning session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetContext {
    pub operation: ClientOperation,
    pub choice: ContextChoice,
}

/// Change native model choices against the original revision and retain one private outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSettings {
    pub operation: ClientOperation,
    pub expected: SettingsRevision,
    pub change: SettingsChange,
    #[serde(default)]
    pub save_default: bool,
}

/// Change the host's tool-label preference without carrying a public session route into Core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetToolLabels {
    pub operation: ClientOperation,
    pub expected: SettingsRevision,
    pub enabled: bool,
    #[serde(default)]
    pub save_default: bool,
}
