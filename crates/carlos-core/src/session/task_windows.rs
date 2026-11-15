//! Address private native task-list windows using the original socket reader and captured scope.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskRevision, TaskWindowId};
use carlos_proto::task_windows::{TaskWindow, MAX_NATIVE_TASK_BYTES, MAX_TASK_WINDOWS};

use super::codex::panel;
use super::codex::panel_state::Snapshot;
use super::state::SessionState;
use super::task_window_state::{Captured, Contents, Reader, Scope, Window};

/// Trusted daemon context; a wire handle alone never grants access to another reader's window.
#[derive(Clone, Debug)]
pub struct WindowContext {
    pub window: TaskWindowId,
    pub reader: Arc<AtomicBool>,
}

/// Read-only window commands share the existing owner mailbox and native read queue.
pub enum WindowCommand {
    Open {
        reader: Arc<AtomicBool>,
        expected: TaskRevision,
        after: Option<TaskWindowId>,
    },
    Read(WindowContext),
    Close {
        reader: Arc<AtomicBool>,
        window: Option<TaskWindowId>,
    },
}

impl SessionState {
    pub(in crate::session) fn open_task_window(
        &mut self,
        reader: Reader,
        expected: &TaskRevision,
        after: Option<&TaskWindowId>,
    ) -> Result<TaskWindow, RpcError> {
        self.expire_task_windows();
        if !reader.load(Ordering::Acquire) || self.codex_scope().closed() {
            return Err(failure("the native task reader has closed"));
        }
        if self.task_windows.slots.len() >= MAX_TASK_WINDOWS {
            return Err(failure(
                "eight native task windows are already retained; close one first",
            ));
        }
        if self.task_window_bytes() >= MAX_NATIVE_TASK_BYTES {
            return Err(failure(
                "native task windows share an 8 MiB allowance; previous page retained",
            ));
        }
        let cursor = if let Some(id) = after {
            let context = WindowContext {
                window: id.clone(),
                reader: reader.clone(),
            };
            let data = self.task_window_capture(&context)?;
            require_revision(expected, &data.revision)?;
            data.older.clone()
        } else {
            require_revision(expected, &self.task_revision(&self.task_rows()))?;
            self.codex_panel.older.clone()
        };
        if cursor.is_null() {
            return Err(failure("no older native task window is available"));
        }
        let mut read = Snapshot::new(self.thread_id(), self.codex_panel.revision, cursor, true);
        read.request = panel::request(self, &read.request.method, read.request.params.clone());
        let mut digest = self.task_scope();
        digest.update(b"nativeTaskWindow");
        digest.update(&read.request.token.to_le_bytes());
        let window: TaskWindowId = digest
            .finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("window digest");
        self.task_windows.slots.insert(
            window.clone(),
            Window {
                reader: reader.clone(),
                scope: Scope::capture(self),
                contents: Contents::Reading(read),
            },
        );
        self.read_task_window(&WindowContext { window, reader })
    }

    pub(in crate::session) fn read_task_window(
        &self,
        context: &WindowContext,
    ) -> Result<TaskWindow, RpcError> {
        let slot = self.task_window_slot(context)?;
        let (revision, rows, reading, older_available, error) = match &slot.contents {
            Contents::Ready(data) => (
                data.revision.clone(),
                data.rows(),
                false,
                !data.older.is_null(),
                data.read_error.clone(),
            ),
            Contents::Reading(_) => (window_revision(&context.window), 0, true, false, None),
            Contents::Failed(error) => (
                window_revision(&context.window),
                0,
                false,
                false,
                Some(error.clone()),
            ),
            Contents::Retired { .. } => return Err(failure("the native task window was closed")),
        };
        Ok(TaskWindow {
            session: self.session_id().clone(),
            window: context.window.clone(),
            seq: self.events.seq,
            revision,
            rows: rows as u32,
            known: matches!(slot.contents, Contents::Ready(_)),
            reading,
            older_available,
            read_error: error,
        })
    }

    pub(in crate::session) fn close_task_windows(
        &mut self,
        reader: &Reader,
        id: Option<&TaskWindowId>,
    ) {
        self.retire_task_windows(reader, id);
        self.prune_task_window_requests();
    }

    pub(in crate::session) fn task_window_capture(
        &self,
        context: &WindowContext,
    ) -> Result<&Captured, RpcError> {
        match &self.task_window_slot(context)?.contents {
            Contents::Ready(data) => Ok(data),
            Contents::Reading(_) => Err(failure("the older native task window is still reading")),
            Contents::Failed(error) => Err(failure(error)),
            Contents::Retired { .. } => Err(failure("the native task window was closed")),
        }
    }

    fn task_window_slot(&self, context: &WindowContext) -> Result<&Window, RpcError> {
        self.task_windows
            .slots
            .get(&context.window)
            .filter(|slot| slot.available(self, &context.reader))
            .ok_or_else(|| failure("retired or foreign native task window; read the latest list"))
    }
}

pub(super) fn window_revision(id: &TaskWindowId) -> TaskRevision {
    id.as_str().parse().expect("window revision digest")
}

pub(super) fn require_revision(
    expected: &TaskRevision,
    actual: &TaskRevision,
) -> Result<(), RpcError> {
    if expected != actual {
        return Err(failure("the task window changed; previous page retained"));
    }
    Ok(())
}

pub(super) fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
