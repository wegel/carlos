//! Keep approval consent local while answering only the current offered wire choice.

use carlos_core::session::entry_id::EntryId;
use carlos_proto::session::{Backend, Decision};
use ratatui::buffer::Buffer;

use super::Device;
use crate::app::overlay_render::draw_approval_card;
use crate::app::view_models::TerminalSize;

impl Device {
    pub(super) fn draw_approval(&self, buffer: &mut Buffer, size: TerminalSize) {
        let Some(card) = self.selected_approval() else {
            return;
        };
        if card.input.is_some()
            || matches!(self.task_backend(), Some(Backend::Claude | Backend::Codex))
        {
            self.draw_question(buffer, size, card);
            return;
        }
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
