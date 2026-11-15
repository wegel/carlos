//! Check captured task-window headers without retaining a second task replica or device selection.

use carlos_proto::ids::{SessionId, TaskWindowId};
use carlos_proto::task_windows::TaskWindow;
use serde_json::Value;

use crate::client_replies::parse;
use crate::ClientError;

pub(crate) fn header(
    session: SessionId,
    window: Option<TaskWindowId>,
    value: Value,
) -> Result<TaskWindow, ClientError> {
    let header: TaskWindow = parse(value)?;
    if header.session != session {
        return Err(ClientError::ForeignSession);
    }
    if window
        .as_ref()
        .is_some_and(|window| window != &header.window)
        || header.rows > 768
        || header.known && header.reading
        || !header.known && header.rows != 0
        || header
            .read_error
            .as_ref()
            .is_some_and(|error| error.len() > 4096)
    {
        return Err(ClientError::InvalidPayload(
            "native task window does not match its original read".into(),
        ));
    }
    Ok(header)
}
