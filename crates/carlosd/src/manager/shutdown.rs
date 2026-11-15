//! Finish captured stop workers and cancelled launches before dropping the daemon's catalogue.

use std::time::{Duration, Instant};

use super::stop_worker::Stopped;
use super::Manager;

impl Manager {
    pub(super) fn shutdown(&mut self) {
        self.startup.stop();
        for state in self.sessions.values() {
            if let Some(cancel) = &state.launch {
                cancel.cancel();
            }
            if state.stopping.is_none() {
                if let Some(live) = &state.live {
                    live.handle.shutdown();
                }
            }
        }
        for peer in self.peers.values() {
            peer.socket.close();
        }
        self.peers.clear();
        // Each cancelled launch retains its slot until its exact completion arrives.
        while self.sessions.values().any(|state| state.launch.is_some()) {
            self.drain_launches();
            let _ = self.wake_rx.recv_timeout(Duration::from_millis(25));
        }
        self.join_stops();
        for state in self.sessions.values_mut() {
            if let Some(live) = state.live.take() {
                live.handle.shutdown();
                let _ = live.handle.join();
                if let Some(notice) = live.notice {
                    let _ = notice.join();
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.drain_saves();
            self.schedule_saves();
            if self.sessions.values().all(|state| state.saving.is_none())
                || Instant::now() >= deadline
            {
                break;
            }
            let _ = self.wake_rx.recv_timeout(Duration::from_millis(25));
        }
    }

    fn join_stops(&mut self) {
        let ids: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, state)| state.stopping.as_ref()?.worker.as_ref().map(|_| id.clone()))
            .collect();
        for id in ids {
            let worker = self
                .sessions
                .get_mut(&id)
                .expect("session")
                .stopping
                .as_mut()
                .expect("stop")
                .worker
                .take()
                .expect("worker");
            let stopped = worker.join().unwrap_or_else(|_| Stopped {
                updates: Vec::new(),
                error: Some("Session stop worker panicked during daemon shutdown".into()),
            });
            self.owner_stopped(&id, stopped);
        }
    }
}
