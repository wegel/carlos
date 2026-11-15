//! Bind child activity interests to a socket's existing session lifetime and read reservations.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use carlos_core::engine::EngineCommand;
use carlos_core::session::task_activity::{ActivityCommand, ActivityContext};
use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::ids::{SessionId, TaskActivityId};

use super::task_windows::Reader;
use super::Manager;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn task_activity_command(&mut self, peer: u64, id: RequestId, command: Command) {
        match self.prepare_activity_command(peer, command) {
            Ok((session, command)) => {
                self.rpc_read(peer, id, session, EngineCommand::TaskActivity(command))
            }
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    fn prepare_activity_command(
        &mut self,
        peer: u64,
        command: Command,
    ) -> Result<(SessionId, ActivityCommand), RpcError> {
        match command {
            command @ Command::TaskActivityOpen { .. } => self.open_activity_command(peer, command),
            Command::TaskActivityRead {
                session,
                activity,
                since,
            } => {
                let context = self.activity_context(peer, &session, activity)?;
                Ok((session, ActivityCommand::Read { context, since }))
            }
            Command::TaskActivityPage {
                session,
                activity,
                expected,
                after,
                limit,
            } => {
                let context = self.activity_context(peer, &session, activity)?;
                Ok((
                    session,
                    ActivityCommand::Page {
                        context,
                        expected,
                        after,
                        limit: usize::from(limit.unwrap_or(128)),
                    },
                ))
            }
            Command::TaskActivityAdvance {
                session,
                activity,
                older,
            } => {
                let context = self.activity_context(peer, &session, activity)?;
                Ok((session, ActivityCommand::Advance { context, older }))
            }
            Command::TaskActivityClose { session, activity } => {
                let reader = self.peers[&peer]
                    .activities
                    .get(&session)
                    .ok_or_else(|| error(-32012, "child reader is already closed"))?
                    .0
                    .clone();
                Ok((session, ActivityCommand::Close { reader, activity }))
            }
            _ => unreachable!("typed child activity request"),
        }
    }

    fn open_activity_command(
        &mut self,
        peer: u64,
        command: Command,
    ) -> Result<(SessionId, ActivityCommand), RpcError> {
        let Command::TaskActivityOpen {
            session,
            task,
            expected,
            window,
        } = command
        else {
            unreachable!("typed child open")
        };
        let window = self.window_context(peer, &session, window)?;
        let connection = self.peers.get_mut(&peer).expect("activity reader");
        if !connection
            .sessions
            .get(&session)
            .is_some_and(|interest| !interest.syncing)
        {
            return Err(error(
                -32012,
                "finish subscribing before opening child activity",
            ));
        }
        let reader = connection
            .activities
            .entry(session.clone())
            .or_insert_with(|| Reader(Arc::new(AtomicBool::new(true))))
            .0
            .clone();
        Ok((
            session,
            ActivityCommand::Open {
                reader,
                task,
                expected,
                window,
            },
        ))
    }

    fn activity_context(
        &self,
        peer: u64,
        session: &SessionId,
        activity: TaskActivityId,
    ) -> Result<ActivityContext, RpcError> {
        let reader = self.peers[&peer]
            .activities
            .get(session)
            .ok_or_else(|| error(-32012, "retired or foreign child activity reader"))?;
        Ok(ActivityContext {
            activity,
            reader: reader.0.clone(),
        })
    }
}
