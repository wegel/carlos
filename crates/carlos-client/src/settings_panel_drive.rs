//! Join only model pages whose revision and numbered facts have reached the accepted session.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;

use crate::panels::settings::{facts_bytes, model_bytes, Fetch, Ready};
use crate::panels::{PanelKind, SettingsCatalog};
use crate::{ChangeSet, Client, Effects};

impl Client {
    pub(crate) fn wake_settings_panel(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.settings else {
            return;
        };
        let Some(fetch) = &panel.fetch else { return };
        let reached = fetch.waiting.as_ref().is_some_and(|page| {
            slot.replica
                .as_ref()
                .is_some_and(|replica| replica.sequence() >= page.seq)
        });
        panel.dirty |= changes.settings || changes.reset || changes.info || reached;
    }

    pub(crate) fn drive_settings_panels(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| {
                slot.panels
                    .settings
                    .as_ref()
                    .is_some_and(|panel| panel.dirty)
            })
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            let panel = self.settings_panel_mut(&session).expect("open settings");
            panel.dirty = false;
            if panel.read.is_some() {
                continue;
            }
            if self.settings_fetch_changed(&session) {
                self.settings_panel_mut(&session)
                    .expect("open settings")
                    .fetch = Some(Fetch::default());
            }
            self.settle_settings_page(&session, effects);
            self.request_settings_page(&session, effects);
            if let Some(panel) = self.settings_panel_mut(&session) {
                panel.recount();
            }
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn settle_settings_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some(page) = self
            .settings_panel_mut(session)
            .and_then(|panel| panel.fetch.as_mut())
            .and_then(|fetch| fetch.waiting.take())
        else {
            return;
        };
        let Some(replica) = self.session(session) else {
            return;
        };
        if replica.sequence() < page.seq {
            self.settings_panel_mut(session)
                .expect("open reader")
                .fetch
                .as_mut()
                .expect("unfinished list")
                .waiting = Some(page);
            return;
        }
        if replica
            .settings()
            .is_none_or(|facts| facts.revision != page.settings.revision)
        {
            self.settings_panel_mut(session).expect("open reader").fetch = Some(Fetch::default());
            return;
        }
        let panel = self.settings_panel_mut(session).expect("open reader");
        let fetch = panel.fetch.as_mut().expect("unfinished list");
        let bytes = fetch.bytes.saturating_add(model_bytes(&page.models));
        if bytes > 8 * 1024 * 1024 || page.offset != fetch.next {
            self.fail_settings_read(
                session,
                "Model catalog exceeds the eight MiB editor budget or changed its interval".into(),
            );
        } else {
            fetch.bytes = bytes;
            fetch.models.extend(page.models);
            fetch.facts = Some(page.settings);
            if let Some(next) = page.next {
                fetch.next = next;
            } else {
                let fetch = panel.fetch.take().expect("complete list");
                let facts = fetch.facts.expect("complete list facts");
                panel.ready = Some(Ready {
                    bytes: fetch.bytes + facts_bytes(&facts),
                    catalog: SettingsCatalog {
                        facts,
                        models: fetch.models,
                    },
                });
                panel.error = None;
            }
        }
        effects.panel_changed(session, PanelKind::Settings);
    }

    fn request_settings_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some(fetch) = self
            .settings_panel(session)
            .and_then(|panel| panel.fetch.as_ref())
            .filter(|fetch| fetch.waiting.is_none())
        else {
            return;
        };
        let command = Command::SettingsRead {
            session: session.clone(),
            expected: fetch.facts.as_ref().map(|facts| facts.revision.clone()),
            offset: fetch.next,
            limit: Some(32),
        };
        match self.read(self.current.expect("ready settings reader"), command) {
            Ok(outgoing) => {
                self.settings_panel_mut(session).expect("open reader").read = Some(outgoing.id());
                effects.outgoing.push(outgoing);
            }
            Err(error) => self.fail_settings_read(session, error.to_string()),
        }
        effects.panel_changed(session, PanelKind::Settings);
    }
}
