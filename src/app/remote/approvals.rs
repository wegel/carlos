//! Keep approval consent local while answering only the current offered wire choice.

use anyhow::Result;
use carlos_core::session::entry_id::EntryId;
use carlos_proto::commands::{ApprovalAnswer, Command};
use carlos_proto::session::Decision;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;

use super::transport::Transport;
use super::Device;
use crate::app::overlay_render::draw_approval_card;
use crate::app::view_models::TerminalSize;

impl Device {
    pub(super) fn approval_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
    ) -> Result<bool> {
        let Some(card) = self.client.session(&self.session).and_then(|replica| {
            replica
                .approvals()
                .iter()
                .find(|card| card.active && !card.submitted)
        }) else {
            return Ok(false);
        };
        if card.input.is_some() {
            return Ok(false);
        }
        if key.modifiers != KeyModifiers::NONE {
            return Ok(false);
        }
        let decision = match key.code {
            KeyCode::Char('y') => Decision::Once,
            KeyCode::Char('n') => Decision::Deny,
            KeyCode::Char('s') => Decision::Session,
            KeyCode::Char('c') => Decision::Cancel,
            _ => return Ok(false),
        };
        if !card.choices.contains(&decision) {
            return Ok(true);
        }
        let Some(link) = link else {
            self.view.status_hint = Some("Disconnected; F5 reconnects".into());
            return Ok(true);
        };
        let identity = EntryId::from_wire(&card.id);
        if decision == Decision::Once
            && card.default_to_no
            && self.view.confirm_allow != Some(identity)
        {
            self.view.confirm_allow = Some(identity);
            self.view.status_hint = Some("Press y again to allow this request".into());
            return Ok(true);
        }
        let approval = card.id.clone();
        let session = self.session.clone();
        let outgoing = self
            .client
            .submit(link.connection, |operation| Command::Approve {
                session,
                operation,
                approval,
                answer: ApprovalAnswer::Decision { decision },
            })?;
        link.send(outgoing)?;
        self.view.confirm_allow = None;
        Ok(true)
    }

    pub(super) fn draw_approval(&self, buffer: &mut Buffer, size: TerminalSize) {
        let Some(card) = self.client.session(&self.session).and_then(|replica| {
            replica
                .approvals()
                .iter()
                .find(|card| card.active && !card.submitted)
        }) else {
            return;
        };
        let mut footer = Vec::new();
        for choice in &card.choices {
            footer.push(match choice {
                Decision::Once => "y accept",
                Decision::Session => "s accept session",
                Decision::Deny => "n decline",
                Decision::Cancel => "c cancel turn",
            });
        }
        let confirmation = (self.view.confirm_allow == Some(EntryId::from_wire(&card.id)))
            .then_some("Press y again to allow this request");
        draw_approval_card(
            buffer,
            size,
            &card.title,
            &card.kind,
            confirmation
                .into_iter()
                .chain(card.details.iter().map(String::as_str)),
            &footer.join("  "),
        );
    }
}
