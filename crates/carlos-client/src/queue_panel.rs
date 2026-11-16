//! Open and target one device's queue reader without owning its editor or transport.

use carlos_proto::ids::{QueueEntryId, SessionId};

use crate::panels::{PanelKind, QueuePanel};
use crate::{Client, ClientError, Effects};

impl Client {
    /// Open queue previews, retaining an optional opaque target across native revisions.
    pub fn open_queue_panel(
        &mut self,
        session: &SessionId,
        selected: Option<QueueEntryId>,
    ) -> Result<Effects, ClientError> {
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        let mut effects = Effects::default();
        if slot.panels.queue.is_none() {
            let mut panel = QueuePanel::default();
            panel.selection = selected.map(|id| id.to_string());
            panel.refresh = true;
            panel.dirty = true;
            panel.recount();
            slot.panels.queue = Some(Box::new(panel));
            effects.panel_changed(session, PanelKind::Queue);
        }
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Close this reader even while offline; late point reads cannot reopen it.
    pub fn close_queue_panel(&mut self, session: &SessionId) -> Effects {
        let mut effects = Effects::default();
        self.cancel_queue_reads(session);
        if let Some(slot) = self.sessions.get_mut(session) {
            if slot.panels.queue.take().is_some() {
                effects.panel_changed(session, PanelKind::Queue);
            }
        }
        effects
    }

    /// Borrow checked previews and read status without cloning loaded rows.
    pub fn queue_panel(&self, session: &SessionId) -> Option<&QueuePanel> {
        self.sessions.get(session)?.panels.queue.as_deref()
    }

    /// Require a current subscription and a settled page from the accepted queue revision.
    pub fn queue_is_current(&self, session: &SessionId) -> bool {
        if !self.current_session_is_ready(session) {
            return false;
        }
        let Some(panel) = self.queue_panel(session) else {
            return false;
        };
        !panel.is_reading()
            && panel.pages.is_settled()
            && self
                .session(session)
                .and_then(|replica| replica.queue())
                .is_some_and(|facts| {
                    facts.known && !facts.reading && panel.pages.revision() == Some(&facts.revision)
                })
    }

    /// Explicitly retry native queue discovery without changing the selected opaque identity.
    pub fn refresh_queue_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        self.cancel_queue_reads(session);
        let panel = self
            .queue_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.refresh = true;
        panel.dirty = true;
        panel.pages.invalidate();
        self.finish_queue_change(session)
    }

    /// Move a device's target through the complete supported queue, reading missing pages as needed.
    pub fn move_queue_panel(
        &mut self,
        session: &SessionId,
        direction: isize,
    ) -> Result<Effects, ClientError> {
        let count = self
            .session(session)
            .and_then(|replica| replica.queue())
            .map_or(0, |facts| facts.count as usize);
        let panel = self
            .queue_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.reads.failure = None;
        panel.pages.navigate(direction, count, &mut panel.selection);
        panel.dirty = true;
        self.finish_queue_change(session)
    }

    pub(crate) fn queue_panel_mut(&mut self, session: &SessionId) -> Option<&mut QueuePanel> {
        self.sessions.get_mut(session)?.panels.queue.as_deref_mut()
    }

    pub(crate) fn cancel_queue_reads(&mut self, session: &SessionId) {
        if let Some(panel) = self.queue_panel_mut(session) {
            let ids = panel.cancel_reads();
            for id in ids {
                self.requests.take(&id);
            }
        }
    }

    pub(crate) fn finish_queue_change(
        &mut self,
        session: &SessionId,
    ) -> Result<Effects, ClientError> {
        self.queue_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?
            .recount();
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Queue);
        self.enforce_fact_budget(session, &mut effects);
        self.drive(&mut effects);
        Ok(effects)
    }
}
