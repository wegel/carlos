//! Orchestrate pure client state while the platform owns sockets, clocks, editors and rendering.

use std::collections::{HashMap, VecDeque};

use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::ids::SessionId;
use carlos_proto::session::Availability;
use carlos_proto::submissions::ClientOperation;

use crate::codec::Incoming;
use crate::pending::{self, Requests};
use crate::{
    Bodies, ChangeSet, ClientError, Connection, ConnectionId, Operations, Outbound, Replica,
    Welcome,
};

/// A platform redraws changed facts without consulting a backend or scanning the whole transcript.
pub struct SessionChanges {
    pub session: SessionId,
    pub changes: ChangeSet,
}

/// Non-transcript panels consume typed one-shot reads and keep their own selection and drafts.
pub enum ReadReply {
    Devices(carlos_proto::devices::Devices),
    PromptSelection(carlos_proto::prompts::PromptSelection),
    PromptText(carlos_proto::prompts::PromptText),
    SavedPage(carlos_proto::startup::SavedPage),
    ContextDefaults(carlos_proto::context_defaults::ContextDefaults),
    TaskActivity(carlos_proto::task_activity::TaskActivityHeader),
    TaskActivityPage(carlos_proto::task_activity::TaskActivityPage),
    TaskWindow(carlos_proto::task_windows::TaskWindow),
    Settings(carlos_proto::settings::SettingsPage),
    Goal(carlos_proto::goals::GoalSnapshot),
    Queue(carlos_proto::queue::QueuePage),
    QueueText(carlos_proto::queue::QueueText),
    TaskSnapshot(carlos_proto::task_facts::TaskSnapshot),
    TaskOverview(carlos_proto::task_overview::TaskOverviewPage),
    TaskOverviewText(carlos_proto::task_overview::TaskOverviewText),
    TaskPage(carlos_proto::tasks::TaskPage),
    TaskDetail(carlos_proto::tasks::TaskDetail),
    TaskText(carlos_proto::tasks::TaskText),
    TaskRun(carlos_proto::tasks::TaskRunSnapshot),
    Source {
        session: SessionId,
        chunk: carlos_proto::source::SourceChunk,
    },
    Output {
        session: SessionId,
        chunk: carlos_proto::output::OutputChunk,
    },
}

/// One request failed without granting permission to resend a possibly written mutation.
pub struct RequestFailure {
    pub id: Option<RequestId>,
    pub error: RpcError,
}

/// Match a one-shot read to its request id; live body notifications have no id.
pub struct ReadResult {
    pub id: Option<RequestId>,
    pub reply: ReadReply,
}

/// One input message yields changed facts and explicit outgoing reads, never automatic mutation replay.
#[derive(Default)]
pub struct Effects {
    pub bodies: Vec<crate::BodyToken>,
    pub outgoing: Vec<Outbound>,
    pub sessions: Vec<SessionChanges>,
    pub operations: Vec<ClientOperation>,
    pub replies: Vec<ReadResult>,
    pub failures: Vec<RequestFailure>,
    pub catalog: bool,
    pub welcome: Option<Welcome>,
    pub panels: Vec<crate::panels::PanelChange>,
}

impl Effects {
    pub(crate) fn panel_changed(&mut self, session: &SessionId, kind: crate::panels::PanelKind) {
        if !self
            .panels
            .iter()
            .any(|change| &change.session == session && change.kind == kind)
        {
            self.panels.push(crate::panels::PanelChange {
                session: session.clone(),
                kind,
            });
        }
    }

    pub(crate) fn read_reply(&mut self, serial: i64, reply: ReadReply) {
        self.replies.push(ReadResult {
            id: Some(RequestId::Integer(serial)),
            reply,
        });
    }
}

pub(crate) struct Slot {
    pub since: Option<carlos_proto::ids::Sequence>,
    pub replica: Option<Replica>,
    pub wanted: bool,
    pub syncing: Option<i64>,
    pub subscribed: Option<ConnectionId>,
    pub refresh: bool,
    pub panels: crate::panels::Panels,
}

/// One device's checked session copies, private rights, request tracker and bounded body windows.
#[derive(Default)]
pub struct Client {
    pub(crate) connection: Connection,
    pub(crate) current: Option<ConnectionId>,
    pub(crate) requests: Requests,
    pub(crate) operations: Operations,
    pub(crate) bodies: Bodies,
    pub(crate) catalog: crate::catalog::Catalog,
    pub(crate) sessions: HashMap<SessionId, Slot>,
    pub(crate) watching: bool,
    pub(crate) needs_watch: bool,
    pub(crate) recover: VecDeque<ClientOperation>,
    pub(crate) detach: VecDeque<SessionId>,
    pub(crate) history: crate::client_history::WaitingHistory,
    pub(crate) watches: crate::client_watches::Watches,
    pub(crate) goals: crate::goal_watches::GoalWatches,
    pub(crate) task_watches: crate::task_watches::TaskWatches,
}

impl Client {
    /// Borrow a session copy; a stale copy remains visibly stale until a complete refresh arrives.
    pub fn session(&self, session: &SessionId) -> Option<&Replica> {
        self.sessions.get(session)?.replica.as_ref()
    }
    /// Report whether this device still wants a session's shared facts, including during refresh.
    pub fn wants_session(&self, session: &SessionId) -> bool {
        self.sessions.get(session).is_some_and(|slot| slot.wanted)
    }
    /// Require an accepted subscription on this greeted connection before using Ready session facts.
    pub fn session_is_ready(&self, connection: ConnectionId, session: &SessionId) -> bool {
        self.session_has_current_snapshot(connection, session)
            && self
                .session(session)
                .is_some_and(|replica| replica.info().availability == Availability::Ready)
    }
    /// Check accepted subscription facts without treating a retained copy as current after reconnect.
    pub fn session_has_current_snapshot(
        &self,
        connection: ConnectionId,
        session: &SessionId,
    ) -> bool {
        self.is_ready(connection)
            && self.sessions.get(session).is_some_and(|slot| {
                slot.wanted
                    && !slot.refresh
                    && slot.syncing.is_none()
                    && slot.subscribed == Some(connection)
                    && slot
                        .replica
                        .as_ref()
                        .is_some_and(|replica| !replica.is_stale())
            })
    }
    /// Let callback consumers inspect accepted Starting facts while preserving reconnect fences.
    pub fn current_session_has_snapshot(&self, session: &SessionId) -> bool {
        self.current
            .is_some_and(|connection| self.session_has_current_snapshot(connection, session))
    }
    /// Let renderers distinguish current Ready facts from a retained copy during reconnect.
    pub fn current_session_is_ready(&self, session: &SessionId) -> bool {
        self.current
            .is_some_and(|connection| self.session_is_ready(connection, session))
    }
    /// Borrow the current shared catalogue without duplicating large transcript rows.
    pub fn catalog(&self) -> impl Iterator<Item = &carlos_proto::session::SessionInfo> {
        self.catalog.iter()
    }
    /// Borrow private results and exact submitted text for this device's own editor choices.
    pub fn operations(&self) -> &Operations {
        &self.operations
    }
    /// Explicitly release a handled result after the platform retained any needed recovery text.
    pub fn forget_operation(&mut self, operation: &ClientOperation) {
        self.operations.forget(operation);
    }
    /// Borrow bounded body windows for a platform renderer.
    pub fn bodies(&self) -> &Bodies {
        &self.bodies
    }
    /// Open or release body windows without exposing mutable core sessions or native commands.
    pub fn bodies_mut(&mut self) -> &mut Bodies {
        &mut self.bodies
    }

    /// Issue a typed read after greeting; the library correlates its eventual reply.
    pub fn read(
        &mut self,
        connection: ConnectionId,
        command: Command,
    ) -> Result<Outbound, ClientError> {
        self.require_ready(connection)?;
        if matches!(
            command,
            Command::SourceSubscribe { .. } | Command::OutputSubscribe { .. }
        ) {
            return Err(ClientError::InvalidPayload(
                "use watch_body with its owned body token".into(),
            ));
        }
        let kind = pending::read_kind(&command)?;
        self.check_interest(&kind)?;
        let outgoing = self.requests.issue(connection, command, kind)?;
        self.record_interest(&outgoing);
        Ok(outgoing)
    }

    /// Consume one checked envelope from its original socket generation.
    pub fn receive(
        &mut self,
        connection: ConnectionId,
        incoming: Incoming,
    ) -> Result<Effects, ClientError> {
        if self.current != Some(connection) {
            return Ok(Effects::default());
        }
        let mut effects = Effects::default();
        match incoming {
            Incoming::Notification(notification) => {
                self.require_ready(connection)?;
                self.notification(*notification, &mut effects)?;
            }
            Incoming::Success { id, result } => {
                if let Some((serial, kind)) = self.requests.take(&id) {
                    self.success(connection, serial, kind, result, &mut effects)?;
                }
            }
            Incoming::Failure { id, error } => {
                let pending = id.as_ref().and_then(|id| self.requests.take(id));
                self.failed(pending, &error, &mut effects);
                effects.failures.push(RequestFailure { id, error });
            }
        }
        self.accept_panel_replies(&mut effects);
        self.drive(&mut effects);
        Ok(effects)
    }

    pub(crate) fn require_ready(&self, connection: ConnectionId) -> Result<(), ClientError> {
        if !self.connection.is_ready(connection) {
            return Err(ClientError::InvalidConnection);
        }
        Ok(())
    }

    pub(crate) fn changed(
        &mut self,
        session: &SessionId,
        changes: ChangeSet,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        self.wake_queue_panel(session, &changes);
        if changes == ChangeSet::default() {
            return Ok(());
        }
        if !self.enforce_fact_budget(session, effects) {
            return Ok(());
        }
        if changes.info || changes.reset {
            if let Some(replica) = self.session(session) {
                self.catalog.update(replica.info().clone())?;
                effects.catalog = true;
            }
        }
        self.reconcile_bodies(session, &changes);
        effects.sessions.push(SessionChanges {
            session: session.clone(),
            changes,
        });
        Ok(())
    }
}
