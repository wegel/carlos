//! Keep native goal polling alive only for connected clients that explicitly read its panel.

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::RequestId;
use carlos_proto::ids::SessionId;
use carlos_proto::session::Backend;

use super::state::ReadKind;
use super::Manager;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn subscribe_goal(&mut self, peer: u64, id: RequestId, session: SessionId) {
        let connection = &self.peers[&peer];
        if !connection.sessions.contains_key(&session) {
            self.fail(
                peer,
                id,
                error(-32012, "subscribe to the session before reading its goal"),
            );
            return;
        }
        if self
            .sessions
            .get(&session)
            .is_none_or(|state| state.info.backend != Backend::Codex)
        {
            self.fail(
                peer,
                id,
                error(-32012, "this backend does not support native goals"),
            );
            return;
        }
        match self.queue_read(
            peer,
            session.clone(),
            ReadKind::Rpc(id.clone()),
            EngineCommand::GoalRead { refresh: true },
        ) {
            Ok(()) => {
                self.peers
                    .get_mut(&peer)
                    .expect("peer")
                    .goals
                    .insert(session);
            }
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    /// Aggregate only live socket interests; a detach or disconnect cannot strand the timer.
    pub(super) fn refresh_goal_interests(&mut self) {
        for peer in self.peers.values_mut() {
            peer.goals
                .retain(|session| peer.sessions.contains_key(session));
        }
        for (session, state) in &self.sessions {
            if let Some(live) = &state.live {
                let active = self.peers.values().any(|peer| peer.goals.contains(session));
                live.handle.set_goal_interest(active);
            }
        }
    }
}
