//! Assemble one complete native queue input before handing its owned text to a platform editor.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::{QueueEntryId, QueueRevision};

/// Complete plain text and its checked native identity, transferred without a second retained copy.
pub struct QueueInput {
    pub entry: QueueEntryId,
    pub expected: QueueRevision,
    pub text: String,
}

pub(crate) struct InputRead {
    pub entry: QueueEntryId,
    pub expected: QueueRevision,
    pub bytes: u64,
    pub text: String,
    pub request: Option<RequestId>,
}

#[derive(Default)]
pub(crate) struct InputState {
    pub read: Option<InputRead>,
    pub ready: Option<QueueInput>,
    pub error: Option<String>,
}

impl InputState {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.read
            .as_ref()
            .map_or(0, |read| read.text.capacity() + 256)
            + self
                .ready
                .as_ref()
                .map_or(0, |ready| ready.text.capacity() + 256)
            + self.error.as_ref().map_or(0, String::len)
    }
}
