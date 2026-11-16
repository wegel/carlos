//! Own checked model-list reads while the platform keeps frozen editor choices and save intent.

use carlos_proto::ids::SessionId;

use crate::panels::settings::Fetch;
use crate::panels::{PanelKind, SettingsCatalog, SettingsPanel};
use crate::{Client, ClientError, Effects};

impl Client {
    /// Open a complete model-list reader after the session's current facts are accepted.
    pub fn open_settings_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        let mut effects = Effects::default();
        if slot.panels.settings.is_none() {
            let mut panel = SettingsPanel::default();
            panel.restart();
            slot.panels.settings = Some(Box::new(panel));
            effects.panel_changed(session, PanelKind::Settings);
        }
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Explicitly discard unfinished pages and reread without touching a platform's editor draft.
    pub fn refresh_settings_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let panel = self
            .settings_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        if let Some(id) = panel.restart() {
            self.requests.take(&id);
        }
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Settings);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow the reader without scanning or cloning its loaded models.
    pub fn settings_panel(&self, session: &SessionId) -> Option<&SettingsPanel> {
        self.sessions.get(session)?.panels.settings.as_deref()
    }

    /// Transfer a completed list to an editor without retaining another full list in this cache.
    pub fn take_settings_catalog(&mut self, session: &SessionId) -> Option<SettingsCatalog> {
        let panel = self.settings_panel_mut(session)?;
        let ready = panel.ready.take()?;
        panel.delivered = true;
        panel.recount();
        Some(ready.catalog)
    }

    /// Retire this reader and its point read even when the transport is offline.
    pub fn close_settings_panel(&mut self, session: &SessionId) -> Effects {
        let mut effects = Effects::default();
        self.cancel_settings_read(session);
        if let Some(slot) = self.sessions.get_mut(session) {
            if slot.panels.settings.take().is_some() {
                effects.panel_changed(session, PanelKind::Settings);
            }
        }
        effects
    }

    pub(crate) fn settings_panel_mut(&mut self, session: &SessionId) -> Option<&mut SettingsPanel> {
        self.sessions
            .get_mut(session)?
            .panels
            .settings
            .as_deref_mut()
    }

    pub(crate) fn cancel_settings_read(&mut self, session: &SessionId) {
        let id = self
            .settings_panel_mut(session)
            .and_then(|panel| panel.read.take());
        if let Some(id) = id {
            self.requests.take(&id);
        }
    }

    pub(crate) fn settings_fetch_changed(&self, session: &SessionId) -> bool {
        self.settings_panel(session)
            .and_then(|panel| panel.fetch.as_ref())
            .and_then(|fetch| fetch.facts.as_ref())
            .zip(self.session(session).and_then(|replica| replica.settings()))
            .is_some_and(|(old, current)| old.revision != current.revision)
    }

    pub(crate) fn fail_settings_read(&mut self, session: &SessionId, message: String) {
        let changed = self.settings_fetch_changed(session);
        if let Some(panel) = self.settings_panel_mut(session) {
            panel.read = None;
            panel.fetch = changed.then(Fetch::default);
            panel.error = (!changed).then_some(message);
            panel.dirty = true;
            panel.recount();
        }
    }
}
