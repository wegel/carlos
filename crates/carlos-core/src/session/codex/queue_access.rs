//! Expose queue facts and explicit snapshot/reset commands.

use serde_json::Value;

use super::queue_state::QueueState;
use crate::session::state::SessionState;

impl SessionState {
    /// Forget the old thread queue while leaving client drafts to the view.
    pub(crate) fn reset_codex_queue(&mut self) {
        self.codex_queue = Default::default();
    }
    /// Borrow queue facts without exposing a mutable controller.
    pub fn codex_queue(&self) -> &QueueState {
        &self.codex_queue
    }
    /// Read the shared native request counter.
    pub(crate) fn codex_serial(&self) -> u64 {
        self.codex_meta.serial
    }
    /// Allocate the next saturated native request token.
    pub(crate) fn advance_codex_serial(&mut self) {
        self.codex_meta.serial = self.codex_meta.serial.saturating_add(1);
    }
    /// Mark the queue stale after a native event or explicit refresh.
    pub fn invalidate_codex_queue(&mut self) {
        self.codex_queue.invalidate();
    }
    /// Clear a queue error when the user refreshes or opens an editor.
    pub fn clear_codex_queue_error(&mut self) {
        self.codex_queue.error = None;
        self.codex_queue.read_error = None;
    }
    /// Install a validated complete reconnect queue without altering a client's selection.
    pub(crate) fn install_codex_queue(&mut self, queue: Option<&Value>) {
        self.codex_queue.entries = queue.and_then(Value::as_array).cloned().unwrap_or_default();
        self.codex_queue.known = queue.is_some();
        self.codex_queue.read = None;
        self.codex_queue.error = queue
            .is_none()
            .then(|| "This server has no queue API".into());
        self.codex_queue.read_error = self.codex_queue.error.clone();
    }
    /// Stop a stale reader after closure without resending its uncertain write.
    pub(crate) fn close_codex_queue(&mut self) {
        self.codex_queue.read = None;
        self.codex_queue.known = false;
    }
    /// Offer submitted text to the client once when the native connection closes.
    pub(crate) fn take_codex_queue_recovery(&mut self) -> Option<String> {
        self.codex_queue
            .operation
            .take()
            .and_then(|mut operation| operation.submitted.take())
    }
}
