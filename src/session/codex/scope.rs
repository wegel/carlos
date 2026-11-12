//! Pass the current native connection identity to moved controllers.

/// Carry the native connection scope while milestone 9 moves its remaining owner.
#[derive(Clone, Copy)]
pub(crate) struct CodexScope {
    pub(crate) enabled: bool,
    pub(crate) closed: bool,
    pub(crate) epoch: u64,
}
