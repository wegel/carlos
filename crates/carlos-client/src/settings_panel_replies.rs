//! Match only owned model-list replies and scope late failures to their original fetched revision.

use crate::panels::PanelKind;
use crate::{Client, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    pub(crate) fn accept_settings_panel_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let ReadReply::Settings(page) = &result.reply else {
            return Some(result);
        };
        let session = page.session.clone();
        let Some(panel) = self.settings_panel_mut(&session) else {
            return Some(result);
        };
        if result.id.is_none() || panel.read != result.id {
            return Some(result);
        }
        panel.read = None;
        if let Some(fetch) = &mut panel.fetch {
            let ReadReply::Settings(page) = result.reply else {
                unreachable!("settings page")
            };
            fetch.waiting = Some(page);
        }
        panel.dirty = true;
        panel.recount();
        effects.panel_changed(&session, PanelKind::Settings);
        None
    }

    pub(crate) fn fail_settings_panel_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let Some(session) = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .settings
                .as_ref()
                .filter(|panel| panel.read == failure.id && failure.id.is_some())
                .map(|_| session.clone())
        }) else {
            return false;
        };
        self.fail_settings_read(&session, failure.error.message.clone());
        effects.panel_changed(&session, PanelKind::Settings);
        true
    }
}
