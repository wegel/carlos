//! Serializes native batches and history continuations without holding native state while sending.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::owned_child::OwnedChild;

use super::event_batch::{BatchGate, BatchPermit};
use super::event_normalizer::EventNormalizer;
use super::event_queue::EventSender;
use super::transport::Pending;
use super::transport_state::{ShutdownRequest, TransportState};

/// Gives each producer one complete batch slot, from state changes through its last send.
pub(super) struct EventEmitter {
    batch: BatchGate,
    events: EventSender,
    owner: Arc<Mutex<EventNormalizer>>,
    child: Arc<Mutex<OwnedChild>>,
    state: Arc<TransportState>,
    pending: Pending,
}

impl EventEmitter {
    /// Share a single batch gate among stdout, stderr, and the history caller.
    pub(super) fn new(
        events: EventSender,
        owner: Arc<Mutex<EventNormalizer>>,
        child: Arc<Mutex<OwnedChild>>,
        state: Arc<TransportState>,
        pending: Pending,
    ) -> Self {
        Self {
            batch: BatchGate::new(events.workers.clone()),
            events,
            owner,
            child,
            state,
            pending,
        }
    }

    /// Normalize and deliver one whole accepted stdout batch before the next producer enters.
    pub(super) fn route(
        self: &Arc<Self>,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
        epoch: u64,
    ) -> Result<()> {
        self.events.checkpoint("route_ready");
        let batch = if EventNormalizer::requires_open_session(frame) {
            let Some(batch) = self.batch.enter_active()? else {
                return Ok(());
            };
            batch
        } else {
            let emitter = self.clone();
            let frame = frame.clone();
            let request = request.map(str::to_string);
            let context = context.clone();
            let Some(batch) = self
                .batch
                .enter_or_defer(frame.to_string().len(), move |batch| {
                    let _ = emitter.route_batch(&frame, request.as_deref(), &context, epoch, batch);
                })?
            else {
                return Ok(());
            };
            batch
        };
        self.route_batch(frame, request, context, epoch, batch)
    }

    fn route_batch(
        &self,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
        epoch: u64,
        batch: BatchPermit,
    ) -> Result<()> {
        let (lines, terminal) = {
            let mut owner = self
                .owner
                .lock()
                .map_err(|_| anyhow!("Claude normalizer lock poisoned"))?;
            let lines = owner.route(frame, request, context, epoch)?;
            if owner.is_closed() {
                batch.mark_terminal();
            }
            (lines, owner.is_closed())
        };
        if terminal {
            self.events.finish(lines, batch, || self.stop());
            Ok(())
        } else {
            self.deliver(lines, batch)
        }
    }

    /// Advance history in the same order as accepted stdout batches and stderr failures.
    pub(super) fn continue_history(&self, epoch: Option<u64>) -> Result<()> {
        self.events.checkpoint("history_ready");
        let Some(batch) = self.batch.enter_active()? else {
            return Ok(());
        };
        let lines = {
            let mut owner = self
                .owner
                .lock()
                .map_err(|_| anyhow!("Claude normalizer lock poisoned"))?;
            if epoch.is_some_and(|epoch| epoch != owner.epoch) {
                return Ok(());
            }
            owner.next_history()
        };
        self.deliver(lines, batch)
    }

    /// Mark helper stdout EOF for deterministic shutdown tests.
    pub(super) fn reader_eof_checkpoint(&self) {
        self.events.checkpoint("reader_eof");
    }

    /// Mark that EOF facts have been ordered, even when their sender still waits for bytes.
    pub(super) fn reader_done_checkpoint(&self) {
        self.events.checkpoint("reader_done");
    }

    /// Publish one terminal failure after any batch already being emitted.
    pub(super) fn failure(self: &Arc<Self>, message: &str) {
        self.events.checkpoint("failure_ready");
        let emitter = self.clone();
        let deferred_message = message.to_string();
        let Ok(Some(batch)) = self.batch.enter_or_defer(message.len(), move |batch| {
            emitter.failure_batch(&deferred_message, batch);
        }) else {
            self.stop();
            return;
        };
        self.failure_batch(message, batch);
    }

    fn failure_batch(&self, message: &str, batch: BatchPermit) {
        let lines = self
            .owner
            .lock()
            .map(|mut owner| owner.failure(message))
            .unwrap_or_default();
        batch.mark_terminal();
        self.events.finish(lines, batch, || self.stop());
    }

    /// Publish EOF after the reader rejects unanswered synchronous calls.
    pub(super) fn exited(self: &Arc<Self>, message: &str) {
        self.finish_exit(message, false);
    }

    /// Stop malformed or overflowing native input before delivering its final facts.
    pub(super) fn stop_and_report(self: &Arc<Self>, message: &str) {
        self.stop();
        self.finish_exit(message, true);
    }

    fn finish_exit(self: &Arc<Self>, message: &str, report_error: bool) {
        let emitter = self.clone();
        let deferred_message = message.to_string();
        let Ok(Some(batch)) = self.batch.enter_or_defer(0, move |batch| {
            emitter.exit_batch(&deferred_message, report_error, batch);
        }) else {
            return;
        };
        self.exit_batch(message, report_error, batch);
    }

    fn exit_batch(&self, message: &str, report_error: bool, batch: BatchPermit) {
        let lines = self
            .owner
            .lock()
            .map(|mut owner| owner.exited(message, report_error))
            .unwrap_or_default();
        if !lines.is_empty() {
            batch.mark_terminal();
            self.events.finish(lines, batch, || self.stop());
        }
    }

    fn deliver(&self, lines: Vec<String>, batch: BatchPermit) -> Result<()> {
        let lines = lines.into_iter();
        for line in lines {
            if self.events.send(line.clone()).is_err() {
                self.stop();
                let failure = self
                    .owner
                    .lock()
                    .map(|mut owner| {
                        owner.exited("Claude UI event backlog overflowed; session stopped", true)
                    })
                    .unwrap_or_default();
                batch.mark_terminal();
                self.events.finish(failure, batch, || self.stop());
                bail!("Claude UI event backlog overflowed; session stopped");
            }
            self.events.checkpoint("batch_line");
        }
        Ok(())
    }

    /// Request child shutdown once, independently of stdout EOF and final-event delivery.
    pub(super) fn stop(&self) {
        self.state.request_shutdown(
            self.child.clone(),
            &self.pending,
            &self.events.workers,
            ShutdownRequest::Terminate,
        );
    }
}
