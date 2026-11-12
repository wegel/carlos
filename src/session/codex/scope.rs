//! Read the current native connection scope without exposing mutable backend fields.

/// Identify the enabled backend and its current root epoch for synchronous native commands.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CodexScope {
    pub(in crate::session) enabled: bool,
    pub(in crate::session) closed: bool,
    pub(in crate::session) epoch: u64,
}

impl CodexScope {
    /// Check whether native Codex handling is enabled.
    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }
    /// Check whether the current native connection has closed.
    pub(crate) fn closed(self) -> bool {
        self.closed
    }
    /// Read the current root epoch.
    pub(crate) fn epoch(self) -> u64 {
        self.epoch
    }
    /// Supply an exact scope to existing terminal-free controller fixtures.
    #[cfg(test)]
    pub(crate) fn for_test(enabled: bool, closed: bool, epoch: u64) -> Self {
        Self {
            enabled,
            closed,
            epoch,
        }
    }
}
