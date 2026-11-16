//! Bound retained facts across sessions and reconcile body windows without rescanning transcripts.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::ids::SessionId;

use crate::client::{Client, Effects, RequestFailure, SessionChanges};
use crate::ChangeSet;

const MAX_FACT_BYTES: usize = 64 * 1024 * 1024;

impl Client {
    pub(crate) fn enforce_fact_budget(
        &mut self,
        session: &SessionId,
        effects: &mut Effects,
    ) -> bool {
        let bytes: usize = self
            .sessions
            .values()
            .map(|slot| {
                slot.replica
                    .as_ref()
                    .map_or(0, |replica| replica.retained_bytes())
                    + slot.panels.retained_bytes()
            })
            .sum();
        if bytes <= MAX_FACT_BYTES {
            return true;
        }
        self.cancel_panel_reads(session);
        if let Some(slot) = self.sessions.get_mut(session) {
            slot.panels = Default::default();
            slot.replica = None;
            slot.wanted = false;
            slot.syncing = None;
            slot.refresh = false;
        }
        self.bodies.forget_session(session);
        effects.panel_changed(session, crate::panels::PanelKind::Queue);
        effects.panel_changed(session, crate::panels::PanelKind::Settings);
        effects.panel_changed(session, crate::panels::PanelKind::Context);
        effects.panel_changed(session, crate::panels::PanelKind::Goal);
        effects.panel_changed(session, crate::panels::PanelKind::TaskDetails);
        effects.panel_changed(session, crate::panels::PanelKind::TaskOverview);
        if !self.detach.contains(session) {
            self.detach.push_back(session.clone());
        }
        effects.sessions.push(SessionChanges {
            session: session.clone(),
            changes: ChangeSet {
                reset: true,
                ..Default::default()
            },
        });
        effects.failures.push(RequestFailure { id: None, error: RpcError {
            code: -32010,
            message: format!("Session {session} exceeds the client's combined 64 MiB fact cache; close another session before reading it again. Private submitted text and backend work remain available."),
        } });
        false
    }

    pub(crate) fn reconcile_bodies(&mut self, session: &SessionId, changes: &ChangeSet) {
        if changes.reset {
            let ids: Vec<_> = self
                .bodies
                .tokens(session)
                .into_iter()
                .map(|token| token.entry().clone())
                .collect();
            self.reconcile_body_ids(session, &ids);
        }
        self.reconcile_body_ids(session, &changes.bodies);
        if changes.removed_front > 0 || changes.removed_back > 0 {
            if let Some(replica) = self
                .sessions
                .get(session)
                .and_then(|slot| slot.replica.as_ref())
            {
                self.bodies
                    .retain_entries(session, |id| replica.entry(id).is_some());
            }
        }
    }

    fn reconcile_body_ids(&mut self, session: &SessionId, ids: &[carlos_proto::ids::EntryId]) {
        for id in ids {
            let facts = self
                .session(session)
                .and_then(|replica| replica.entry(id))
                .map(|entry| {
                    (
                        entry.source.clone(),
                        entry.tool.as_ref().map(|tool| tool.output_revision),
                    )
                });
            match facts {
                Some((Some(source), _)) => self.bodies.observe_source(session, id, &source),
                Some((None, Some(revision))) => self.bodies.observe_revision(session, id, revision),
                _ => self.bodies.forget(session, id),
            }
        }
    }

    pub(crate) fn drain_detach(&mut self, effects: &mut Effects) -> bool {
        let Some(connection) = self.current else {
            return false;
        };
        while let Some(session) = self.detach.front().cloned() {
            match self.read(connection, Command::Unsubscribe { session }) {
                Ok(outgoing) => {
                    self.detach.pop_front();
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return false,
            }
        }
        true
    }
}
