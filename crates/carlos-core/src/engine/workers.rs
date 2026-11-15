//! Run bounded native waits without borrowing the session owner.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use carlos_proto::submissions::ClientOperation;

use crate::session::command_work::BackendCompletion;
use crate::session::control_work::ControlCompletion;
use crate::session::ralph::RalphConfig;

pub(super) enum Completion {
    ContextRead {
        pending: Box<super::mailbox::Envelope>,
        result: Result<carlos_proto::context_defaults::ContextDefaults, String>,
    },
    ContextSaved {
        operation: ClientOperation,
        result: Result<(), String>,
    },
    ToolLabelsSaved {
        operation: ClientOperation,
        error: Option<String>,
    },
    BackendSettings {
        operation: ClientOperation,
        result: crate::backend_commands::SettingsResult,
    },
    SettingsSaved {
        operation: ClientOperation,
        error: Option<String>,
    },
    TaskStopDispatch {
        operation: ClientOperation,
        result: Result<(), String>,
    },
    Ordinary {
        operation: Option<ClientOperation>,
        completion: BackendCompletion,
    },
    Control {
        operation: ClientOperation,
        completion: ControlCompletion,
    },
    InternalControl(ControlCompletion),
    Ralph {
        operation: ClientOperation,
        result: Result<RalphConfig, String>,
        target: super::target::Target,
    },
    Action {
        target: super::target::Target,
        result: Result<bool, super::actions::ActionFailure>,
        kind: super::actions::ActionKind,
    },
}

type Job = Box<dyn FnOnce() -> Completion + Send>;

pub(super) struct Pool {
    sender: Option<mpsc::SyncSender<Job>>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Pool {
    pub fn new(
        count: usize,
        completed: mpsc::SyncSender<Completion>,
        wake: mpsc::SyncSender<()>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(count);
        let receiver = Arc::new(Mutex::new(receiver));
        let threads = (0..count)
            .map(|_| {
                let receiver = receiver.clone();
                let completed = completed.clone();
                let wake = wake.clone();
                thread::spawn(move || loop {
                    let job = receiver.lock().expect("worker queue").recv();
                    let Ok(job) = job else {
                        break;
                    };
                    if completed.send(job()).is_err() {
                        break;
                    }
                    let _ = wake.try_send(());
                })
            })
            .collect();
        Self {
            sender: Some(sender),
            threads,
        }
    }

    pub fn submit(&self, job: impl FnOnce() -> Completion + Send + 'static) {
        // The owner counts queued plus executing jobs before it submits to this pool.
        self.sender
            .as_ref()
            .expect("live worker pool")
            .try_send(Box::new(job))
            .unwrap_or_else(|_| panic!("owner exceeded worker admission"));
    }

    pub fn close(&mut self) {
        self.sender.take();
    }
    fn join_until(&mut self, deadline: Instant) {
        for thread in self.threads.drain(..) {
            while !thread.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
            // The owner has already retired work and stopped native transports. A stuck file read
            // owns no session borrow; its eventual completion cannot revive or write to the session.
        }
    }
}

pub(super) struct Workers {
    pub ordinary: Pool,
    pub controls: Pool,
    pub actions: Pool,
    pub completed: mpsc::Receiver<Completion>,
    pub ordinary_busy: bool,
    pub controls_busy: usize,
    pub actions_busy: usize,
}

impl Workers {
    pub fn new(wake: &mpsc::SyncSender<()>) -> Self {
        let (sender, completed) = mpsc::sync_channel(16);
        Self {
            ordinary: Pool::new(1, sender.clone(), wake.clone()),
            controls: Pool::new(4, sender.clone(), wake.clone()),
            actions: Pool::new(2, sender, wake.clone()),
            completed,
            ordinary_busy: false,
            controls_busy: 0,
            actions_busy: 0,
        }
    }

    pub fn stop(&mut self) {
        self.ordinary.close();
        self.controls.close();
        self.actions.close();
        let deadline = Instant::now() + Duration::from_secs(1);
        self.ordinary.join_until(deadline);
        self.controls.join_until(deadline);
        self.actions.join_until(deadline);
    }
}
