//! Check bounded native forms off the UI thread without launching another runtime.

use std::sync::{mpsc, Arc};
use std::thread;

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use crate::native_form;
use crate::protocol_state::ClientState;

struct Job {
    id: Value,
    revision: u64,
    schema: Value,
    answer: Option<Value>,
}

/// Keep one linear-regex worker and a bounded queue for native callback checks.
pub(crate) struct FormWorker {
    sender: mpsc::SyncSender<Job>,
}

impl FormWorker {
    /// Start the worker with the same lifetime and output budget as its native process.
    pub(crate) fn start(state: Arc<ClientState>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(64);
        thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                if state.is_closed() {
                    break;
                }
                let result = match job.answer {
                    Some(answer) => native_form::validate(&job.schema, &answer),
                    None => native_form::compile(&job.schema).map(|_| ()),
                };
                let result = json!({"method":"carlos/codex/formChecked","params":{"requestId":job.id,
                    "revision":job.revision,"ok":result.is_ok(),"error":result.err().map(|error|error.to_string())}});
                if !state.is_closed() {
                    if let Err(error) = state.events.send(result.to_string()) {
                        state.close(&error.to_string(), true);
                    }
                }
            }
        });
        Self { sender }
    }

    /// Reject oversized work before it can occupy the queue or UI callback state.
    pub(crate) fn check(
        &self,
        id: Value,
        schema: Value,
        answer: Option<Value>,
        revision: u64,
    ) -> Result<()> {
        ensure!(
            (id.is_string() || id.is_number()) && id.to_string().len() <= 4096,
            "Invalid or oversized form callback ID"
        );
        ensure!(
            schema.to_string().len() <= 65536,
            "Form schema exceeds 64 KiB"
        );
        ensure!(
            answer
                .as_ref()
                .is_none_or(|answer| answer.to_string().len() <= 65536),
            "Form answer exceeds 64 KiB"
        );
        self.sender
            .try_send(Job {
                id,
                revision,
                schema,
                answer,
            })
            .map_err(|_| anyhow::anyhow!("Native form validation queue is full or closed"))
    }
}
