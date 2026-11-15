//! Admit owner commands and capture final private outcomes before an explicit shutdown.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

use carlos_proto::commands::RpcError;
use carlos_proto::ids::Sequence;

use super::{EngineCommand, EngineHandle, EngineReply, StopRequest};

impl EngineHandle {
    /// Admit a bounded request without waiting for native work or a socket write.
    pub fn request(&self, command: EngineCommand) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let mailbox = match &command {
            EngineCommand::Reopen { .. }
            | EngineCommand::Send { .. }
            | EngineCommand::SetRalph { .. }
            | EngineCommand::ToggleRalph { .. }
            | EngineCommand::Rename { .. }
            | EngineCommand::Rewind { .. } => &self.0.ordinary,
            EngineCommand::GoalSet { .. } | EngineCommand::GoalClear { .. } => &self.0.ordinary,
            EngineCommand::QueueChange { .. }
            | EngineCommand::ContextSet { .. }
            | EngineCommand::SettingsSet { .. }
            | EngineCommand::ToolLabelsSet { .. } => &self.0.ordinary,
            EngineCommand::TaskStop { .. }
            | EngineCommand::Reconnect { .. }
            | EngineCommand::Interrupt { .. }
            | EngineCommand::Approve { .. } => &self.0.control,
            _ => &self.0.reads,
        };
        let reply = mailbox.request(command)?;
        let _ = self.0.wake.try_send(());
        Ok(reply)
    }

    /// Enable the existing native goal timer while at least one connected client reads goals.
    pub fn set_goal_interest(&self, active: bool) {
        if self.0.goal_interest.swap(active, Ordering::SeqCst) != active {
            let _ = self.0.wake.try_send(());
        }
    }

    /// Keep the existing three-second native task refresh alive for connected visible list readers.
    pub fn set_task_interest(&self, active: bool) {
        if self.0.task_interest.swap(active, Ordering::SeqCst) != active {
            let _ = self.0.wake.try_send(());
        }
    }

    /// Distinguish fact/result changes from a wake caused only by a completed read.
    pub fn take_updates(&self) -> bool {
        self.0.changed.swap(false, Ordering::SeqCst)
    }

    /// Stop the session explicitly; dropping a device socket never calls this.
    pub fn shutdown(&self) {
        self.0.stop.store(true, Ordering::SeqCst);
        let _ = self.0.wake.try_send(());
    }

    /// Stop this owner, interrupt its attached native turn, and return the final outcomes after cleanup.
    pub fn stop_session(&self, since: Sequence) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let mut stop_request = self.0.stop_request.lock().expect("session stop request");
        if stop_request.is_some()
            || self.0.stop.load(Ordering::SeqCst)
            || self.0.stopping.load(Ordering::SeqCst)
        {
            return Err(RpcError {
                code: -32012,
                message: "session shutdown was already requested".into(),
            });
        }
        let (reply, received) = mpsc::sync_channel(1);
        *stop_request = Some(StopRequest { since, reply });
        self.0.stopping.store(true, Ordering::SeqCst);
        let _ = self.0.wake.try_send(());
        Ok(received)
    }

    /// Wait for the already requested bounded native shutdown.
    pub fn join(&self) -> thread::Result<()> {
        if let Some(thread) = self.0.thread.lock().expect("engine join").take() {
            thread.join()
        } else {
            Ok(())
        }
    }
}
