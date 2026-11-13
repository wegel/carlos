//! Keep queued and executing requests within the same count and byte budget.

use std::sync::{mpsc, Arc, Mutex};

use carlos_proto::commands::RpcError;

use super::{EngineCommand, EngineReply};

struct Usage {
    count: usize,
    bytes: usize,
}
struct Budget {
    usage: Mutex<Usage>,
    count: usize,
    bytes: usize,
}

pub(super) struct Permit {
    budget: Arc<Budget>,
    bytes: usize,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut usage = self.budget.usage.lock().expect("engine admission budget");
        usage.count -= 1;
        usage.bytes -= self.bytes;
    }
}

pub(super) struct Envelope {
    pub(super) command: EngineCommand,
    pub(super) reply: mpsc::SyncSender<EngineReply>,
    pub(super) permit: Permit,
}

pub(super) struct Mailbox {
    sender: mpsc::SyncSender<Envelope>,
    budget: Arc<Budget>,
}

impl Mailbox {
    pub(super) fn new(count: usize, bytes: usize) -> (Self, mpsc::Receiver<Envelope>) {
        let (sender, receiver) = mpsc::sync_channel(count);
        (
            Self {
                sender,
                budget: Arc::new(Budget {
                    usage: Mutex::new(Usage { count: 0, bytes: 0 }),
                    count,
                    bytes,
                }),
            },
            receiver,
        )
    }

    pub(super) fn request(
        &self,
        command: EngineCommand,
    ) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let bytes = command.bytes();
        let mut usage = self.budget.usage.lock().expect("engine admission budget");
        if usage.count >= self.budget.count || bytes > self.budget.bytes.saturating_sub(usage.bytes)
        {
            return Err(full());
        }
        usage.count += 1;
        usage.bytes += bytes;
        drop(usage);
        let permit = Permit {
            budget: self.budget.clone(),
            bytes,
        };
        let (reply, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Envelope {
                command,
                reply,
                permit,
            })
            .map_err(|_| full())?;
        Ok(receiver)
    }
}

pub(super) fn full() -> RpcError {
    RpcError {
        code: -32010,
        message: "session command queue is full or closed; nothing dispatched".into(),
    }
}
