//! Forward native input with bounded storage and apply each report before releasing its charge.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::owner::Owner;
use crate::backend::EventReceiver;
use crate::session::backend_update::BackendChange;

pub(super) struct Native {
    pub receiver: Option<mpsc::Receiver<String>>,
    pub thread: Option<thread::JoinHandle<()>>,
    pub close: Option<Box<dyn FnOnce() + Send>>,
    pub closed: bool,
}

impl Native {
    pub fn start(mut events: EventReceiver, channels: &super::owner::Channels) -> Self {
        let close = events.take_close();
        let (sender, receiver) = mpsc::sync_channel(64);
        let stop = channels.stop.clone();
        let wake = channels.wake.clone();
        let thread = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match events.recv_timeout(Duration::from_millis(100)) {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                        let _ = wake.try_send(());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            drop(sender);
            let _ = wake.try_send(());
        });
        Self {
            receiver: Some(receiver),
            thread: Some(thread),
            close,
            closed: false,
        }
    }

    pub fn stop(&mut self) {
        self.receiver.take();
        if let Some(close) = self.close.take() {
            close();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Owner {
    pub(super) fn drain_native(&mut self) {
        for _ in 0..64 {
            match self
                .native
                .receiver
                .as_ref()
                .expect("native receiver")
                .try_recv()
            {
                Ok(line) => self.receive_native(&line),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !self.native.closed {
                        self.close_native();
                    }
                    break;
                }
            }
        }
    }

    fn receive_native(&mut self, line: &str) {
        let context = self.session.backend_context();
        if let Some(mut batch) =
            self.session
                .receive_native_line(self.client.as_ref(), line, &context, None)
        {
            let mut installed = false;
            while let Some(update) = batch.next(&mut self.session, self.client.as_ref(), &context) {
                installed |= matches!(update.change, BackendChange::Installed(_));
                self.report(update);
            }
            if installed {
                self.finish_native_reconnect();
            }
            return;
        }
        let handled = self.client.handle_event(line);
        match handled {
            Some(lines) => {
                for line in lines {
                    let update = self.session.handle_backend_line(&line, &context);
                    self.report(update);
                }
            }
            None => {
                let update = self.session.handle_backend_line(line, &context);
                self.report(update);
            }
        }
        self.client.acknowledge_event(line.len());
        if self.session.events.backend_closed && !self.native.closed {
            self.close_native();
        }
    }

    pub(super) fn close_native(&mut self) {
        self.native.closed = true;
        self.session.task_stops.clear();
        self.retire_settings_scope();
        self.retire_goal_scope();
        self.retire_queue_scope();
        self.retire_task_stops();
        self.session.events.backend_closed = true;
        self.session.retire_backend_command();
        self.session.set_active_turn_id(None);
        self.session.ralph_stop_after_failure();
        for draft in self.session.ralph_take_queued_human_drafts() {
            self.results
                .recover(draft, "backend closed before queued text was sent", false);
        }
        self.notice();
    }
}
