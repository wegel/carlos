//! Keep a session and its receiver on one thread while bounded reads remain independent.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use carlos_proto::commands::RpcError;

use super::mailbox::Envelope;
use super::ring::{self, EventRing};
use super::{EngineChanges, EngineCommand, EngineReply};
use crate::backend::{BackendClient, EventReceiver};
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;

pub(super) struct Channels {
    pub ordinary: mpsc::Receiver<Envelope>,
    pub control: mpsc::Receiver<Envelope>,
    pub reads: mpsc::Receiver<Envelope>,
    pub wake: mpsc::SyncSender<()>,
    pub wake_rx: mpsc::Receiver<()>,
    pub notice: mpsc::SyncSender<()>,
    pub stop: Arc<AtomicBool>,
    pub stopping: Arc<AtomicBool>,
    pub stop_request: Arc<Mutex<Option<super::StopRequest>>>,
    pub changed: Arc<AtomicBool>,
    pub goal_interest: Arc<AtomicBool>,
    pub task_interest: Arc<AtomicBool>,
}

pub(super) struct Owner {
    pub(super) tool_labels: Option<mpsc::Receiver<crate::summaries::SummaryEvent>>,
    pub(super) tool_labels_save: Option<carlos_proto::submissions::ClientOperation>,
    pub(super) task_activity_reads: Vec<super::mailbox::Envelope>,
    pub(super) task_window_reads: Vec<super::mailbox::Envelope>,
    pub(super) task_stops:
        HashMap<carlos_proto::submissions::ClientOperation, super::task_stops::PendingTaskStop>,
    pub(super) queue: Option<super::queue_changes::PendingQueue>,
    pub(super) goal: Option<super::goals::PendingGoal>,
    pub(super) settings: Option<super::settings::PendingSettings>,
    pub(super) reopening: bool,
    pub session: SessionState,
    pub client: Box<dyn BackendClient>,
    pub ring: EventRing,
    pub channels: Channels,
    pub(super) native: super::native::Native,
    pub(super) workers: super::workers::Workers,
    pub(super) results: super::results::Results,
    pub(super) ordinary: VecDeque<EngineCommand>,
    pub(super) actions: super::actions::Actions,
}

impl Owner {
    pub fn new(
        session: SessionState,
        client: Box<dyn BackendClient>,
        events: EventReceiver,
        channels: Channels,
    ) -> Self {
        let native = super::native::Native::start(events, &channels);
        let workers = super::workers::Workers::new(&channels.wake);
        Self {
            tool_labels: None,
            tool_labels_save: None,
            task_activity_reads: Vec::new(),
            task_window_reads: Vec::new(),
            task_stops: HashMap::new(),
            queue: None,
            goal: None,
            settings: None,
            reopening: false,
            session,
            client,
            native,
            workers,
            channels,
            ring: EventRing::default(),
            results: Default::default(),
            ordinary: VecDeque::new(),
            actions: Default::default(),
        }
    }

    pub fn run(mut self) {
        self.publish();
        while !self.channels.stop.load(Ordering::SeqCst)
            && !self.channels.stopping.load(Ordering::SeqCst)
        {
            self.drain_native();
            self.drain_tool_labels();
            self.drain_completions();
            self.retire_settings_scope();
            self.retire_goal_scope();
            self.retire_queue_scope();
            self.retire_task_stops();
            self.dispatch_controls();
            self.admit_ordinary();
            self.dispatch_actions();
            self.tick();
            self.dispatch_ordinary();
            self.publish();
            self.finish_task_window_reads();
            self.finish_task_activity_reads();
            for _ in 0..64 {
                let Ok(envelope) = self.channels.reads.try_recv() else {
                    break;
                };
                let Some(envelope) = self.defer_task_window_read(envelope) else {
                    continue;
                };
                let Some(envelope) = self.defer_task_activity_read(envelope) else {
                    continue;
                };
                let Some(envelope) = self.defer_context_read(envelope) else {
                    continue;
                };
                let reply = self.read(envelope.command);
                let _ = envelope.reply.try_send(reply);
                let _ = self.channels.notice.try_send(());
            }
            let _ = self.channels.wake_rx.recv_timeout(self.wait());
        }
        self.finish_shutdown();
    }

    fn finish_shutdown(&mut self) {
        let stop = self
            .channels
            .stop_request
            .lock()
            .expect("session stop request")
            .take();
        let native = if stop.is_some() {
            self.stop_attached_turn()
        } else {
            Ok(())
        };
        self.close_native();
        self.client.stop();
        self.channels.stop.store(true, Ordering::SeqCst);
        self.native.stop();
        self.workers.stop();
        self.drain_completions();
        self.session.clear_codex_callbacks();
        for queue in [
            crate::session::approval_state::ApprovalQueue::Claude,
            crate::session::approval_state::ApprovalQueue::Codex,
            crate::session::approval_state::ApprovalQueue::Grok,
        ] {
            let _ = self.session.clear_approval_queue(queue);
        }
        let _ = self.session.retire_active_approval();
        self.session.events.backend_closed = true;
        self.session.set_active_turn_id(None);
        self.publish();
        self.finish_stop_request(stop, native);
    }

    fn finish_stop_request(
        &mut self,
        stop: Option<super::StopRequest>,
        native: Result<(), RpcError>,
    ) {
        if let Some(stop) = stop {
            let report = EngineReply::Stopped {
                update: Box::new(self.updates(None)),
                native,
            };
            let _ = stop.reply.try_send(report);
        }
    }

    pub(super) fn notice(&self) {
        self.channels.changed.store(true, Ordering::SeqCst);
        let _ = self.channels.notice.try_send(());
    }

    pub(super) fn publish(&mut self) {
        if self.session.event_availability() == carlos_proto::session::Availability::Ready {
            if let Some(name) = self.session.events.launch_name.take() {
                self.session.set_thread_name(Some(name));
            }
        }
        if self.session.events.ralph_restore_notice
            && self.session.event_availability() == carlos_proto::session::Availability::Ready
        {
            self.session.events.ralph_restore_notice = false;
            self.session.append_message(crate::session::models::Role::System, "Ralph was on before the daemon stopped. It remains off; explicitly enable it to resume automatic work.");
        }
        if self.ring.publish(&mut self.session) {
            self.notice();
        }
    }

    fn read(&mut self, command: EngineCommand) -> EngineReply {
        let result = match command {
            EngineCommand::Info => Ok(EngineReply::Info {
                info: self.session.export_info(),
                native_thread: self.session.thread_id().clone(),
            }),
            EngineCommand::Snapshot { limit } => ring::snapshot(&self.session, limit)
                .map(|value| EngineReply::Snapshot(Box::new(value))),
            EngineCommand::Changes { since } => match since.and_then(|seq| self.ring.replay(seq)) {
                Some(frames) => Ok(EngineReply::Changes(EngineChanges::Replay {
                    frames,
                    seq: self.ring.sequence(),
                })),
                None => ring::snapshot(&self.session, 100)
                    .map(|value| EngineReply::Changes(EngineChanges::Snapshot(Box::new(value)))),
            },
            EngineCommand::History { before, limit } => self.history(&before, limit),
            EngineCommand::Output {
                entry,
                cursor,
                limit,
            } => self
                .session
                .sealed()
                .output_chunk(&entry, cursor.as_ref(), limit)
                .map(EngineReply::Output)
                .map_err(|error| failure(error.to_string())),
            EngineCommand::Source {
                entry,
                cursor,
                limit,
            } => self
                .session
                .sealed()
                .source_chunk(&entry, cursor.as_ref(), limit)
                .map(EngineReply::Source)
                .map_err(|error| failure(error.to_string())),
            EngineCommand::Updates { since } => {
                Ok(EngineReply::Updates(Box::new(self.updates(Some(since)))))
            }
            EngineCommand::SourceAt {
                entry,
                revision,
                offset,
                limit,
            } => self
                .session
                .sealed()
                .source_at(&entry, revision, offset, limit)
                .map(EngineReply::Source)
                .map_err(|error| failure(error.to_string())),
            EngineCommand::Outcomes => Ok(EngineReply::Outcomes(self.results.collect())),
            command => self.read_native(command),
        };
        result.unwrap_or_else(EngineReply::Failure)
    }

    fn updates(&mut self, since: Option<carlos_proto::ids::Sequence>) -> super::EngineUpdate {
        let changes = match since.and_then(|seq| self.ring.replay(seq)) {
            Some(frames) => Ok(EngineChanges::Replay {
                frames,
                seq: self.ring.sequence(),
            }),
            None => ring::snapshot(&self.session, 100)
                .map(|snapshot| EngineChanges::Snapshot(Box::new(snapshot))),
        };
        super::EngineUpdate {
            seq: self.ring.sequence(),
            info: self.session.export_info(),
            native_thread: self.session.thread_id().clone(),
            native_resume: self.client.native_resume_kind(self.session.thread_id()),
            changes,
            outcomes: self.results.collect(),
        }
    }

    fn history(
        &mut self,
        before: &carlos_proto::ids::EntryId,
        limit: usize,
    ) -> Result<EngineReply, RpcError> {
        let end = self
            .session
            .messages
            .position(EntryId::from_wire(before))
            .ok_or_else(|| failure("retired or foreign history cursor"))?;
        let start = crate::session::event_rows::page_start(&self.session.messages()[..end], limit);
        ring::check_rows(&self.session.messages()[start..end])?;
        let history = self
            .session
            .sealed()
            .history(before, limit)
            .expect("checked history cursor");
        Ok(EngineReply::History(history))
    }
}

pub(super) fn failure(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
