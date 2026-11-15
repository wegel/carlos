//! Submit explicit Ralph intent after local editors have consumed their own keys.

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::session::SessionInfo;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::transport::Transport;
use super::Device;
use crate::app::status_line::Label;
use crate::theme::COLOR_GUTTER_AGENT_THINKING;

impl Device {
    pub(super) fn ralph_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> bool {
        if key.code != KeyCode::Char('r') || !key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if let Err(error) = self.toggle_ralph(link) {
            self.view.status_hint = Some(format!("ralph: {error}"));
        }
        true
    }

    /// Submit a private Ralph toggle and let the daemon check its native goal conflict.
    pub(super) fn toggle_ralph(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.goals.ralph.is_some() {
            bail!("Ralph toggle pending");
        }
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        if !self.client.session_is_ready(link.connection, &self.session) {
            bail!("Waiting for fresh ready session facts; Ralph request was not sent");
        }
        self.view.status_hint = None;
        self.view.codex_goals.error = None;
        let session = self.session.clone();
        let on = !self
            .client
            .session(&session)
            .is_some_and(|replica| replica.info().ralph);
        let options = on.then(|| Box::new(self.ralph_options.clone()));
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            Command::ToggleRalph {
                session,
                operation,
                options,
            }
        })?;
        self.goals.ralph = submitted;
        link.send(outgoing)
    }
}

pub(super) fn ralph_badges(info: &SessionInfo) -> Vec<Label> {
    let mut badges = Vec::new();
    if info
        .status
        .as_ref()
        .is_some_and(|status| status.ralph.toggle_pending)
    {
        badges.push(Label::plain(
            "Ralph pending".into(),
            COLOR_GUTTER_AGENT_THINKING,
        ));
    }
    if info.ralph {
        badges.push(Label {
            parts: vec![("RALPH MODE".into(), COLOR_GUTTER_AGENT_THINKING)],
            bold: true,
        });
    }
    badges
}
