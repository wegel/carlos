//! Plain context-window token counts.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Contextusage.
pub struct ContextUsage {
    pub used: u64,
    pub max: u64,
}
