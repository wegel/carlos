//! Plain context-window token counts.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Contextusage.
pub(crate) struct ContextUsage {
    pub(crate) used: u64,
    pub(crate) max: u64,
}
