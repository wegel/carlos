//! Install each captured launch once, or stop its late owner before releasing the slot.

use std::thread;

use carlos_core::engine::launch::{LaunchError, Launched};
use carlos_proto::ids::SessionId;
use carlos_proto::session::Availability;
use carlos_proto::submissions::OperationState;

use super::state::Live;
use super::stop_worker::{self, Stopped};
use super::Manager;

impl Manager {
    pub(super) fn drain_launches(&mut self) {
        while let Ok((id, result)) = self.launches.try_recv() {
            self.sessions
                .get_mut(&id)
                .expect("captured launch session")
                .launch = None;
            let result = result.and_then(|launched| self.install_launched(&id, launched));
            if let Err(reason) = result {
                if self.sessions[&id].stopping.is_some() {
                    self.owner_stopped(
                        &id,
                        Stopped {
                            updates: Vec::new(),
                            error: None,
                        },
                    );
                } else {
                    self.launch_failed(&id, reason);
                }
            }
            self.list_changed(&id);
            self.resume_subscriptions(&id);
        }
    }

    fn install_launched(
        &mut self,
        id: &SessionId,
        mut launched: Launched,
    ) -> Result<(), LaunchError> {
        let session = self.sessions.get_mut(id).expect("known launch");
        session.info = launched.info();
        let native_thread = launched.native_thread().to_owned();
        // A late successful launch still owns native resources. Stop and join it normally.
        session.saved.native_thread = Some(native_thread.clone());
        let (handle, notices) = launched.spawn_after(session.last_seq)?;
        let wake = self.wake.clone();
        let notice = thread::spawn(move || {
            while notices.recv().is_ok() {
                let _ = wake.try_send(());
            }
        });
        let mut live = Live {
            native_thread,
            handle,
            notice: Some(notice),
            updates: None,
            seq: session.last_seq,
            dirty: true,
        };
        if let Some(stop) = &mut session.stopping {
            stop.worker = Some(stop_worker::start(&mut live, self.wake.clone()));
        }
        session.live = Some(live);
        Ok(())
    }

    fn launch_failed(&mut self, id: &SessionId, reason: LaunchError) {
        let session = self.sessions.get_mut(id).expect("known session");
        session.info.availability = Availability::Unavailable;
        for mut result in std::mem::take(&mut session.create) {
            result.state = match &reason {
                LaunchError::Invalid(message) => OperationState::Rejected {
                    message: message.clone(),
                    recovery: None,
                },
                reason => OperationState::Uncertain {
                    message: reason.to_string(),
                    recovery: None,
                },
            };
            self.result(result);
        }
    }
}
