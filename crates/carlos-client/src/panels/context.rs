//! Keep one wanted host-default read separate from live model usage and private editor text.

use carlos_proto::commands::RequestId;
use carlos_proto::context_defaults::ContextDefaults;

/// Read status and the host's saved size for its next native process, never the active model's size.
#[derive(Default)]
pub struct ContextPanel {
    pub(crate) request: Option<RequestId>,
    pub(crate) waiting: bool,
    pub(crate) result: Option<Result<ContextDefaults, String>>,
}

impl ContextPanel {
    /// Distinguish a wanted or pending read from a completed result.
    pub fn is_reading(&self) -> bool {
        self.waiting || self.request.is_some()
    }

    /// Borrow a checked host default before an editor takes its saved-choice baseline.
    pub fn defaults(&self) -> Option<&ContextDefaults> {
        self.result.as_ref().and_then(|result| result.as_ref().ok())
    }

    /// Borrow this read's error without changing any private draft or live token counter.
    pub fn read_error(&self) -> Option<&str> {
        self.result
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .map(String::as_str)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        256 + self.read_error().map_or(0, str::len)
    }

    pub(crate) fn disconnected(&mut self) {
        self.request = None;
        self.result = None;
        self.waiting = true;
    }
}
