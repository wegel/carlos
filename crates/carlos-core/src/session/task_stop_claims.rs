//! Retire queued task controls at their core root and native process changes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskRun};
use carlos_proto::submissions::ClientOperation;

use super::codex::panel_state::Process;
use super::state::SessionState;
use super::task_stop_target::StopTarget;

#[derive(Debug, Default)]
pub(in crate::session) struct TaskStopClaims {
    entries: HashMap<ClientOperation, Guard>,
}

/// The session owns retirement while the prepared command shares its checked native target.
#[derive(Debug)]
pub(in crate::session) struct StopClaim {
    pub task: TaskId,
    pub run: TaskRun,
    pub target: StopTarget,
    pub live: AtomicBool,
}

#[derive(Debug)]
struct Guard(Arc<StopClaim>);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.live.store(false, Ordering::SeqCst);
    }
}

impl TaskStopClaims {
    pub fn begin(
        &mut self,
        operation: ClientOperation,
        task: TaskId,
        run: TaskRun,
        target: StopTarget,
    ) -> Result<Arc<StopClaim>, RpcError> {
        if self.entries.len() >= 8 || self.entries.contains_key(&operation) {
            return Err(failure(
                "Carlos already has eight pending task stops or this operation is already pending",
            ));
        }
        if self
            .entries
            .values()
            .any(|claim| claim.0.task == task && claim.0.run == run)
        {
            return Err(failure("a stop for this exact task run is already pending"));
        }
        let claim = Arc::new(StopClaim {
            task,
            run,
            target,
            live: AtomicBool::new(true),
        });
        self.entries.insert(operation, Guard(claim.clone()));
        Ok(claim)
    }

    pub fn finish(&mut self, operation: &ClientOperation) {
        self.entries.remove(operation);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn refresh_processes(&self, session: &SessionState) {
        for claim in self.entries.values() {
            if !claim.0.target.process_is_current(session) {
                claim.0.live.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn retire_process_owner(&self, owner: &str) {
        for claim in self.entries.values() {
            if matches!(&claim.0.target, StopTarget::Process { thread, .. } if thread == owner) {
                claim.0.live.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn reconcile_processes(&self, owner: Option<&str>, rows: &[Process]) {
        for claim in self.entries.values() {
            if let StopTarget::Process { thread, .. } = &claim.0.target {
                if owner.is_none_or(|owner| thread == owner)
                    && !rows.iter().any(|row| claim.0.target.matches_process(row))
                {
                    claim.0.live.store(false, Ordering::SeqCst);
                }
            }
        }
    }
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
