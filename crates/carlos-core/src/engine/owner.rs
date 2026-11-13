//! Keep a session and its receiver on one thread while bounded reads remain independent.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

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
}

pub(super) struct Owner {
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
        while !self.channels.stop.load(Ordering::SeqCst) {
            self.drain_native();
            self.drain_completions();
            self.dispatch_controls();
            self.admit_ordinary();
            self.dispatch_actions();
            self.tick();
            self.dispatch_ordinary();
            self.publish();
            for _ in 0..64 {
                let Ok(envelope) = self.channels.reads.try_recv() else {
                    break;
                };
                let reply = self.read(envelope.command);
                let _ = envelope.reply.try_send(reply);
            }
            let _ = self.channels.wake_rx.recv_timeout(self.wait());
        }
        self.close_native();
        self.client.stop();
        self.channels.stop.store(true, Ordering::SeqCst);
        self.native.stop();
        self.workers.stop();
        self.drain_completions();
        self.publish();
    }

    pub(super) fn notice(&self) {
        let _ = self.channels.notice.try_send(());
    }

    pub(super) fn publish(&mut self) {
        if self.ring.publish(&mut self.session) {
            let _ = self.channels.notice.try_send(());
        }
    }

    fn read(&mut self, command: EngineCommand) -> EngineReply {
        let result = match command {
            EngineCommand::Info => Ok(EngineReply::Info(self.session.export_info())),
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
            EngineCommand::Outcomes => Ok(EngineReply::Outcomes(self.results.collect())),
            _ => Err(failure("mutation entered the read lane")),
        };
        result.unwrap_or_else(EngineReply::Failure)
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
        let start = end.saturating_sub(limit.clamp(1, 1000));
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
