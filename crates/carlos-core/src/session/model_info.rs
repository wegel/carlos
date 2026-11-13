//! Describe backend models without depending on protocol clients.

/// Describe one model and its supported reasoning efforts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub model: String,
    pub display_name: String,
    pub supported_efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub is_default: bool,
}
