//! Correlate bounded requests without retaining another copy of their submitted text.

use std::collections::HashMap;

use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::submissions::ClientOperation;

use crate::codec;
use crate::{BodyToken, ClientError, ConnectionId};

#[derive(Clone)]
pub(crate) enum Kind {
    Hello,
    Watch,
    Subscribe(SessionId),
    Unsubscribe(SessionId),
    History {
        session: SessionId,
        before: EntryId,
        limit: usize,
    },
    Mutation(ClientOperation),
    Operation(ClientOperation),
    Goal(SessionId),
    Settings(crate::settings_reads::SettingsTarget),
    GoalWatch(SessionId),
    GoalUnwatch(SessionId),
    Queue {
        session: SessionId,
        expected: Option<carlos_proto::ids::QueueRevision>,
    },
    QueueText(QueueTextTarget),
    TaskWatch(SessionId),
    TaskUnwatch(SessionId),
    TaskOverview(crate::task_overview::OverviewTarget),
    TaskOverviewText(crate::task_overview::OverviewTextTarget),
    TaskPage(crate::tasks::TaskPageTarget),
    TaskDetail(crate::tasks::TaskTarget),
    TaskText(crate::tasks::TaskTextTarget),
    TaskRun(crate::tasks::TaskRunTarget),
    TaskWindow {
        session: SessionId,
        window: Option<carlos_proto::ids::TaskWindowId>,
    },
    Source(BodyTarget),
    Output(BodyTarget),
    BodyUnsubscribe {
        session: SessionId,
        entry: EntryId,
        source: bool,
    },
    Unit,
}

#[derive(Clone)]
pub(crate) struct BodyTarget {
    pub session: SessionId,
    pub entry: EntryId,
    pub token: Option<BodyToken>,
    pub watch: bool,
}

#[derive(Clone)]
pub(crate) struct QueueTextTarget {
    pub limit: usize,
    pub session: SessionId,
    pub expected: carlos_proto::ids::QueueRevision,
    pub entry: carlos_proto::ids::QueueEntryId,
    pub offset: carlos_proto::bytes::ByteOffset,
}

pub(crate) struct Pending {
    pub kind: Kind,
    bytes: usize,
}

/// Only platform code writes these bytes; Debug deliberately omits private greeting credentials.
pub struct Outbound {
    pub(crate) connection: ConnectionId,
    pub(crate) id: i64,
    bytes: Vec<u8>,
}

impl Outbound {
    /// Tag write callbacks and received messages with their original transport generation.
    pub fn connection(&self) -> ConnectionId {
        self.connection
    }
    /// Match a platform write failure to the request that had been handed out.
    pub fn id(&self) -> RequestId {
        RequestId::Integer(self.id)
    }
    /// Borrow one complete JSON-RPC request, including its Unix newline delimiter.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Transfer owned bytes to a platform writer without cloning submitted text.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Default)]
pub(crate) struct Requests {
    entries: HashMap<i64, Pending>,
    bytes: usize,
    counter: i64,
}

impl Requests {
    pub fn issue(
        &mut self,
        connection: ConnectionId,
        command: Command,
        kind: Kind,
    ) -> Result<Outbound, ClientError> {
        if self.entries.len() >= 128 {
            return Err(ClientError::Capacity(
                "outstanding client requests are full",
            ));
        }
        let id = self
            .counter
            .checked_add(1)
            .ok_or(ClientError::Capacity("request counter exhausted"))?;
        let bytes = codec::encode(&codec::request(RequestId::Integer(id), command))?;
        if bytes.len() > (4 * 1024 * 1024usize).saturating_sub(self.bytes) {
            return Err(ClientError::Capacity("outstanding request bytes are full"));
        }
        self.counter = id;
        self.bytes += bytes.len();
        self.entries.insert(
            id,
            Pending {
                kind,
                bytes: bytes.len(),
            },
        );
        Ok(Outbound {
            connection,
            id,
            bytes,
        })
    }

    pub fn take(&mut self, id: &RequestId) -> Option<(i64, Kind)> {
        let RequestId::Integer(id) = id else {
            return None;
        };
        let pending = self.entries.remove(id)?;
        self.bytes -= pending.bytes;
        Some((*id, pending.kind))
    }

    pub fn has_watch(&self, token: &BodyToken) -> bool {
        self.entries.values().any(|pending| match &pending.kind {
            Kind::Source(target) | Kind::Output(target) => {
                target.watch && target.token.as_ref() == Some(token)
            }
            _ => false,
        })
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn kind(&self, id: i64) -> Option<&Kind> {
        self.entries.get(&id).map(|pending| &pending.kind)
    }
}

pub(crate) fn read_kind(command: &Command) -> Result<Kind, ClientError> {
    Ok(match command {
        Command::Watch {} => Kind::Watch,
        Command::Subscribe { session, .. } => Kind::Subscribe(session.clone()),
        Command::Unsubscribe { session } => Kind::Unsubscribe(session.clone()),
        Command::History {
            session,
            before,
            limit,
        } => Kind::History {
            session: session.clone(),
            before: before.clone(),
            limit: usize::from(*limit).clamp(1, 1000),
        },
        Command::Operation { operation } => Kind::Operation(operation.clone()),
        Command::TaskSubscribe { session } => Kind::TaskWatch(session.clone()),
        Command::TaskUnsubscribe { session } => Kind::TaskUnwatch(session.clone()),
        Command::TaskWindowOpen { session, .. } => Kind::TaskWindow {
            session: session.clone(),
            window: None,
        },
        Command::TaskWindowRead { session, window } => Kind::TaskWindow {
            session: session.clone(),
            window: Some(window.clone()),
        },
        Command::TaskWindowClose { .. } => Kind::Unit,
        overview @ (Command::TaskOverview { .. } | Command::TaskOverviewText { .. }) => {
            overview_kind(overview)
        }
        Command::SettingsRead {
            session,
            expected,
            offset,
            limit,
        } => Kind::Settings(crate::settings_reads::SettingsTarget {
            session: session.clone(),
            expected: expected.clone(),
            offset: *offset,
            limit: usize::from(limit.unwrap_or(32))
                .clamp(1, carlos_proto::settings::MAX_MODELS_PER_PAGE),
        }),
        Command::GoalRead { session, .. } => Kind::Goal(session.clone()),
        Command::GoalSubscribe { session } => Kind::GoalWatch(session.clone()),
        Command::GoalUnsubscribe { session } => Kind::GoalUnwatch(session.clone()),
        queue @ (Command::QueueRead { .. } | Command::QueueText { .. }) => queue_kind(queue),
        task @ (Command::TaskList { .. }
        | Command::TaskDetail { .. }
        | Command::TaskText { .. }
        | Command::TaskRun { .. }) => crate::tasks::read_kind(task),
        body @ (Command::Source { .. }
        | Command::SourceSeek { .. }
        | Command::SourceSubscribe { .. }
        | Command::Output { .. }
        | Command::OutputSubscribe { .. }
        | Command::SourceUnsubscribe { .. }
        | Command::OutputUnsubscribe { .. }) => body_kind(body),
        _ => {
            return Err(ClientError::InvalidPayload(
                "use submit for mutations and begin for hello".into(),
            ))
        }
    })
}

fn overview_kind(command: &Command) -> Kind {
    match command {
        Command::TaskOverview {
            session,
            expected,
            after,
            limit,
        } => Kind::TaskOverview(crate::task_overview::OverviewTarget {
            session: session.clone(),
            expected: expected.clone(),
            after: after.clone(),
            limit: usize::from(limit.unwrap_or(32)).clamp(1, 32),
        }),
        Command::TaskOverviewText {
            session,
            expected,
            entry,
            field,
            offset,
            limit,
        } => Kind::TaskOverviewText(crate::task_overview::OverviewTextTarget {
            session: session.clone(),
            expected: expected.clone(),
            entry: entry.clone(),
            field: *field,
            offset: *offset,
            limit: limit.unwrap_or(65536).clamp(4, 65536) as usize,
        }),
        _ => unreachable!("typed task overview"),
    }
}

fn queue_kind(command: &Command) -> Kind {
    match command {
        Command::QueueRead {
            session, expected, ..
        } => Kind::Queue {
            session: session.clone(),
            expected: expected.clone(),
        },
        Command::QueueText {
            session,
            expected,
            entry,
            offset,
            limit,
        } => Kind::QueueText(QueueTextTarget {
            session: session.clone(),
            expected: expected.clone(),
            entry: entry.clone(),
            offset: *offset,
            limit: limit.unwrap_or(65536).clamp(4, 65536) as usize,
        }),
        _ => unreachable!("typed queue read"),
    }
}

fn body_kind(command: &Command) -> Kind {
    match command {
        Command::Source { session, entry, .. }
        | Command::SourceSeek { session, entry, .. }
        | Command::SourceSubscribe { session, entry, .. } => Kind::Source(BodyTarget {
            session: session.clone(),
            entry: entry.clone(),
            token: None,
            watch: matches!(command, Command::SourceSubscribe { .. }),
        }),
        Command::Output { session, entry, .. }
        | Command::OutputSubscribe { session, entry, .. } => Kind::Output(BodyTarget {
            session: session.clone(),
            entry: entry.clone(),
            token: None,
            watch: matches!(command, Command::OutputSubscribe { .. }),
        }),
        Command::SourceUnsubscribe { session, entry }
        | Command::OutputUnsubscribe { session, entry } => Kind::BodyUnsubscribe {
            session: session.clone(),
            entry: entry.clone(),
            source: matches!(command, Command::SourceUnsubscribe { .. }),
        },
        _ => unreachable!("typed body request"),
    }
}
