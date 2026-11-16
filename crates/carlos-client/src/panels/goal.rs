//! Keep goal read status around the single accepted goal facts without owning an editor or timer.

use carlos_proto::commands::RequestId;
use carlos_proto::goals::GoalSnapshot;

/// One wanted goal reader; the checked goal itself remains in the session's Replica.
#[derive(Default)]
pub struct GoalPanel {
    pub(crate) read: Option<RequestId>,
    pub(crate) future: Option<GoalSnapshot>,
    pub(crate) error: Option<String>,
    pub(crate) refresh: bool,
    pub(crate) dirty: bool,
}

impl GoalPanel {
    /// Distinguish a pending native read or unreached numbered reply from current goal facts.
    pub fn is_reading(&self) -> bool {
        self.read.is_some() || self.future.is_some() || self.refresh
    }

    /// Borrow this reader's failure without replacing a private draft or mutation error.
    pub fn read_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        256 + self.error.as_ref().map_or(0, String::len)
            + self.future.as_ref().map_or(0, |snapshot| {
                let facts = &snapshot.goal;
                facts
                    .goal
                    .as_ref()
                    .map_or(0, |goal| goal.objective.len() + goal.status.len())
                    + facts.notice.len()
                    + facts.read_error.as_ref().map_or(0, String::len)
                    + 512
            })
    }

    pub(crate) fn disconnected(&mut self) {
        self.read = None;
        self.future = None;
        self.dirty = true;
    }
}
