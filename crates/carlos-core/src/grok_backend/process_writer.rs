//! Bound ACP input bytes and wait time without locking the Grok event owner.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::owned_child::OwnedChild;
use crate::write_timing::{Stage, WriteTiming};

use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};

use super::process_callbacks::{Callback, Callbacks};

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_QUEUED: usize = 16 * 1024 * 1024;
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
struct Job {
    frames: Vec<Frame>,
    permission: DispatchGuard,
    reply: mpsc::Sender<Result<()>>,
    timing: Arc<WriteTiming>,
    skip_retired: bool,
}

struct Frame {
    line: String,
    callback: Option<Arc<Callback>>,
    cancellation: bool,
}
type PendingWrite = (mpsc::Receiver<Result<()>>, Arc<WriteTiming>);

pub(super) struct InputWriter {
    sender: mpsc::SyncSender<Job>,
    queued: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
    child: Arc<Mutex<OwnedChild>>,
    callbacks: Arc<Callbacks>,
}

impl InputWriter {
    pub(super) fn start(
        mut stdin: impl Write + Send + 'static,
        child: Arc<Mutex<OwnedChild>>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(8);
        let queued = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let bytes = queued.clone();
        let stopped = closed.clone();
        let callbacks = Arc::new(Callbacks::default());
        let targets = callbacks.clone();
        let owned_child = child.clone();
        thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                job.timing.mark(Stage::Dequeued);
                let length = job
                    .frames
                    .iter()
                    .map(|frame| frame.line.len())
                    .sum::<usize>();
                let result = write_frames(&mut stdin, &job, &stopped, &targets);
                if result
                    .as_ref()
                    .is_err_and(|error| !error.is::<DispatchRetired>())
                    && !stopped.swap(true, Ordering::SeqCst)
                {
                    targets.close();
                    let _ = owned_child.lock().expect("Grok child").kill();
                }
                bytes.fetch_sub(length, Ordering::SeqCst);
                let _ = job.reply.send(result);
            }
        });
        Self {
            sender,
            queued,
            closed,
            child,
            callbacks,
        }
    }

    #[cfg(test)]
    pub(super) fn write(&self, frame: &Value) -> Result<()> {
        self.write_guarded(frame, DispatchGuard::default())
    }

    pub(super) fn write_guarded(&self, frame: &Value, permission: DispatchGuard) -> Result<()> {
        self.await_write(self.enqueue_timed(frame, permission)?)
    }

    fn await_write(&self, pending: PendingWrite) -> Result<()> {
        let (receiver, timing) = pending;
        match receiver.recv_timeout(WRITE_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.is::<DispatchRetired>() => Err(error),
            other => {
                let timing = timing.to_string();
                self.close();
                match other {
                    Ok(Err(error)) => bail!("write Grok stdin: {error}"),
                    _ => bail!("Grok stdin did not finish within 500 ms; request may already have been accepted ({timing})"),
                }
            }
        }
    }

    pub(super) fn callbacks(&self) -> Arc<Callbacks> {
        self.callbacks.clone()
    }

    pub(super) fn write_callback(
        &self,
        callback: Arc<Callback>,
        body: Value,
        error: bool,
        permission: DispatchGuard,
    ) -> Result<()> {
        let frame = callback_frame(callback, body, error);
        self.await_write(self.enqueue_frames(vec![frame], permission, false)?)
    }

    pub(super) fn enqueue_callback(
        &self,
        callback: Arc<Callback>,
        body: Value,
        error: bool,
    ) -> Result<()> {
        let frame = callback_frame(callback, body, error);
        self.enqueue_frames(vec![frame], DispatchGuard::default(), true)?;
        Ok(())
    }

    pub(super) fn cancel_callbacks(
        &self,
        session: Option<&str>,
        epoch: Option<u64>,
    ) -> (Vec<String>, Result<()>) {
        let records = self.callbacks.cancel_session(session, epoch);
        let tokens = records
            .iter()
            .map(|callback| callback.token().to_owned())
            .collect();
        let frames: Vec<_> = records
            .into_iter()
            .map(|callback| {
                callback_frame(callback, json!({"outcome":{"outcome":"cancelled"}}), false)
            })
            .collect();
        let result = if frames.is_empty() {
            Ok(())
        } else {
            // The reader may enqueue cancellation, but never waits for the native pipe.
            self.enqueue_frames(frames, DispatchGuard::default(), true)
                .map(|_| ())
        };
        (tokens, result)
    }

    #[cfg(test)]
    pub(super) fn enqueue(
        &self,
        frame: &Value,
        permission: DispatchGuard,
    ) -> Result<mpsc::Receiver<Result<()>>> {
        self.enqueue_timed(frame, permission)
            .map(|(receiver, _)| receiver)
    }

    fn enqueue_timed(&self, frame: &Value, permission: DispatchGuard) -> Result<PendingWrite> {
        permission.check()?;
        ensure!(!self.closed.load(Ordering::SeqCst), "Grok stdin is closed");
        let frame = Frame {
            line: format!("{frame}\n"),
            callback: None,
            cancellation: false,
        };
        self.enqueue_frames(vec![frame], permission, false)
    }

    fn enqueue_frames(
        &self,
        frames: Vec<Frame>,
        permission: DispatchGuard,
        skip_retired: bool,
    ) -> Result<PendingWrite> {
        permission.check()?;
        ensure!(!self.closed.load(Ordering::SeqCst), "Grok stdin is closed");
        ensure!(
            frames.iter().all(|frame| frame.line.len() <= MAX_FRAME),
            "Grok request exceeds 8 MiB"
        );
        let length = frames.iter().map(|frame| frame.line.len()).sum::<usize>();
        if self
            .queued
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |bytes| {
                bytes.checked_add(length).filter(|sum| *sum <= MAX_QUEUED)
            })
            .is_err()
        {
            self.close();
            bail!("Grok stdin backlog exceeds 16 MiB");
        }
        let (reply, receiver) = mpsc::channel();
        let timing = Arc::new(WriteTiming::new());
        if self
            .sender
            .try_send(Job {
                frames,
                permission,
                reply,
                timing: timing.clone(),
                skip_retired,
            })
            .is_err()
        {
            self.queued.fetch_sub(length, Ordering::SeqCst);
            self.close();
            bail!("Grok stdin queue is full or closed");
        }
        Ok((receiver, timing))
    }

    pub(super) fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            self.callbacks.close();
            let _ = self.child.lock().expect("Grok child").kill();
        }
    }
}

fn callback_frame(callback: Arc<Callback>, body: Value, error: bool) -> Frame {
    let cancellation = error || body["outcome"]["outcome"] == "cancelled";
    let mut value = json!({"jsonrpc":"2.0", "id":callback.id()});
    value[if error { "error" } else { "result" }] = body;
    Frame {
        line: format!("{value}\n"),
        callback: Some(callback),
        cancellation,
    }
}

fn write_frames(
    stdin: &mut impl Write,
    job: &Job,
    closed: &AtomicBool,
    callbacks: &Callbacks,
) -> Result<()> {
    for frame in &job.frames {
        job.permission.check()?;
        ensure!(!closed.load(Ordering::SeqCst), "Grok stdin is closed");
        job.timing.mark(Stage::Checked);
        if let Some(callback) = &frame.callback {
            if let Err(error) = callbacks.dispatch(callback, frame.cancellation) {
                if job.skip_retired {
                    continue;
                }
                return Err(error.into());
            }
        }
        job.timing.mark(Stage::Ready);
        let result = stdin
            .write_all(frame.line.as_bytes())
            .and_then(|()| stdin.flush());
        job.timing.mark(Stage::Returned);
        if let Some(callback) = &frame.callback {
            callbacks.finish(callback.token());
        }
        result?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "writer_guard_tests.rs"]
mod guard_tests;

#[cfg(test)]
#[path = "process_reply_tests.rs"]
mod reply_tests;
