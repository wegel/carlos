//! Backend choices shared by clients; editor drafts and device display choices stay local.

use serde::{Deserialize, Serialize};

use crate::ids::SettingsRevision;

/// Missing values preserve the backend's inherited or unknown choice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeChoices {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub summary: Option<String>,
}

/// One advertised model and the reasoning efforts that its backend offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model: String,
    pub display_name: String,
    pub supported_efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub is_default: bool,
}

/// Small numbered facts invalidate stale settings reads without resending the model catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsFacts {
    pub revision: SettingsRevision,
    pub current: RuntimeChoices,
    pub pending: RuntimeChoices,
    pub supports_effort: bool,
    pub supports_summary: bool,
    pub model_count: u32,
    pub native_pending: bool,
    pub legacy_overrides: bool,
    pub label: String,
    pub summaries_enabled: bool,
}
