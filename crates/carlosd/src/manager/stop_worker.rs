//! Join only the captured owner off the manager and retain both final result channels.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use carlos_core::engine::{EngineHandle, EngineReply, EngineUpdate};

use super::state::Live;

pub(super) struct Stopped {
    pub updates: Vec<EngineUpdate>,
    pub error: Option<String>,
}

pub(super) fn start(live: &mut Live, wake: mpsc::SyncSender<()>) -> JoinHandle<Stopped> {
    let handle = live.handle.clone();
    let final_reply = handle.stop_session();
    let previous = live.updates.take();
    let notice = live.notice.take();
    thread::spawn(move || {
        let stopped = collect(handle, previous, final_reply, notice);
        let _ = wake.try_send(());
        stopped
    })
}

fn collect(
    handle: EngineHandle,
    previous: Option<mpsc::Receiver<EngineReply>>,
    final_reply: Result<mpsc::Receiver<EngineReply>, carlos_proto::commands::RpcError>,
    notice: Option<JoinHandle<()>>,
) -> Stopped {
    let mut stopped = Stopped {
        updates: Vec::new(),
        error: None,
    };
    if final_reply.is_err() {
        handle.shutdown();
    }
    if handle.join().is_err() {
        stopped.error = Some("Session owner panicked during stop".into());
    }
    if notice.is_some_and(|thread| thread.join().is_err()) {
        stopped.error = Some("Session notification worker panicked during stop".into());
    }
    if let Some(EngineReply::Updates(update)) = previous.and_then(|reply| reply.try_recv().ok()) {
        stopped.updates.push(*update);
    }
    match final_reply {
        Ok(reply) => match reply.try_recv() {
            Ok(EngineReply::Stopped { update, native }) => {
                stopped.updates.push(*update);
                if let Err(reason) = native {
                    stopped.error = Some(reason.message);
                }
            }
            _ => stopped.error = Some("Session stopped without a final owner report".into()),
        },
        Err(reason) => stopped.error = Some(reason.message),
    }
    stopped
}
