//! Describe backend models without depending on protocol clients.

/// Describe one model and its supported reasoning efforts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelInfo {
    pub(crate) model: String,
    pub(crate) display_name: String,
    pub(crate) supported_efforts: Vec<String>,
    pub(crate) default_effort: Option<String>,
    pub(crate) is_default: bool,
}
