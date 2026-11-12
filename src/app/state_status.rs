//! Keep local notices separate from backend status while retaining the last writer.

use super::state::AppState;

impl AppState {
    /// Show a notice from this terminal without changing session facts.
    pub(super) fn set_hint(&mut self, text: impl Into<String>) {
        self.view.status_hint = Some((self.session.status_revision(), text.into()));
    }

    // HEAD stores this text but does not draw it in the status bar. Keep its
    // last-writer rule available to parity tests without adding terminal text.
    #[cfg(test)]
    /// Read HEAD's last-writer text for cross-owner parity checks.
    pub(super) fn status_text(&self) -> &String {
        self.view
            .status_hint
            .as_ref()
            .filter(|(revision, _)| *revision == self.session.status_revision())
            .map(|(_, text)| text)
            .unwrap_or_else(|| self.session.status())
    }
}
