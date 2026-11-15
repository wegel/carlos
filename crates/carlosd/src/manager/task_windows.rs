//! Bind private native task windows to one socket's existing session lifetime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_core::engine::EngineCommand;
use carlos_core::session::task_windows::{WindowCommand, WindowContext};
use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::ids::{SessionId, TaskWindowId};

use super::Manager;
use crate::transport::wire::error;

pub(super) struct Reader(pub Arc<AtomicBool>);

impl Drop for Reader {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Manager {
    pub(super) fn task_command_context(
        &self,
        peer: u64,
        command: &Command,
    ) -> Result<Option<WindowContext>, RpcError> {
        match command {
            Command::TaskList {
                session, window, ..
            }
            | Command::TaskDetail {
                session, window, ..
            }
            | Command::TaskText {
                session, window, ..
            }
            | Command::TaskRun {
                session, window, ..
            }
            | Command::TaskStop {
                session, window, ..
            } => self.window_context(peer, session, window.clone()),
            _ => Ok(None),
        }
    }
    pub(super) fn window_context(
        &self,
        peer: u64,
        session: &SessionId,
        window: Option<TaskWindowId>,
    ) -> Result<Option<WindowContext>, RpcError> {
        let Some(window) = window else {
            return Ok(None);
        };
        let reader = self.peers[&peer]
            .task_windows
            .get(session)
            .ok_or_else(|| error(-32012, "retired or foreign native task window"))?;
        Ok(Some(WindowContext {
            window,
            reader: reader.0.clone(),
        }))
    }

    pub(super) fn task_window_command(&mut self, peer: u64, id: RequestId, command: Command) {
        let result = self.prepare_window_command(peer, command);
        match result {
            Ok((session, command)) => {
                self.rpc_read(peer, id, session, EngineCommand::TaskWindow(command))
            }
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    fn prepare_window_command(
        &mut self,
        peer: u64,
        command: Command,
    ) -> Result<(SessionId, WindowCommand), RpcError> {
        match command {
            Command::TaskWindowOpen {
                session,
                expected,
                after,
            } => {
                let connection = self.peers.get_mut(&peer).expect("window reader");
                if !connection
                    .sessions
                    .get(&session)
                    .is_some_and(|interest| !interest.syncing)
                {
                    return Err(error(
                        -32012,
                        "finish subscribing before opening older native task windows",
                    ));
                }
                let reader = connection
                    .task_windows
                    .entry(session.clone())
                    .or_insert_with(|| Reader(Arc::new(AtomicBool::new(true))))
                    .0
                    .clone();
                Ok((
                    session,
                    WindowCommand::Open {
                        reader,
                        expected,
                        after,
                    },
                ))
            }
            Command::TaskWindowRead { session, window } => {
                let context = self
                    .window_context(peer, &session, Some(window))?
                    .expect("window target");
                Ok((session, WindowCommand::Read(context)))
            }
            Command::TaskWindowClose { session, window } => {
                let reader = self.peers[&peer]
                    .task_windows
                    .get(&session)
                    .ok_or_else(|| error(-32012, "native task windows are already closed"))?
                    .0
                    .clone();
                Ok((session, WindowCommand::Close { reader, window }))
            }
            _ => unreachable!("typed native task window"),
        }
    }
}
