//! Paint checked callback descriptions with the same borrowed question card and answer caret.

use carlos_core::session::text::{safe_label, safe_text};
use carlos_proto::session::{Approval, ApprovalInput, Backend, Decision};
use ratatui::buffer::Buffer;

use super::Device;
use crate::app::claude_question_view::{draft_lines, QuestionControls};
use crate::app::overlay_render::question_card::{self, QuestionCard};
use crate::app::view_models::TerminalSize;

impl Device {
    pub(super) fn draw_question(&self, buffer: &mut Buffer, size: TerminalSize, card: &Approval) {
        let Some(draft) = self.questions.drafts.get(&card.id) else {
            return;
        };
        let mut body = draft_lines(&draft.editor);
        if !self.question_facts_are_current() {
            body.push(
                "Waiting for accepted session facts; F5 reconnects. This private answer draft is kept".into(),
            );
        }
        if card.submitted {
            body.push("An answer to this original request is pending; this terminal's draft stays private".into());
        }
        if self.task_backend() == Some(Backend::Codex) && card.kind == "question" {
            body.push("Alt+C/F10 interrupts this query and leaves its question pending until native resolution".into());
        }
        if let Some(ApprovalInput::Browser { url }) = &card.input {
            body.push(format!("Link: {}", safe_label(url)));
            body.push(
                "Ctrl+Y copies this offered URL; accepting sends only browser consent".into(),
            );
        }
        body.push(format!("Request: {}", card.id));
        body.extend(card.details.iter().map(|line| safe_text(line)));
        question_card::draw(
            buffer,
            size,
            QuestionCard {
                title: &card.title,
                caller: card.caller.as_deref().unwrap_or(&card.kind),
                body: &body,
                count: self
                    .client
                    .session(&self.session)
                    .map_or(0, |replica| replica.approvals().len()),
                controls: self.question_controls(card),
            },
            &draft.editor,
        );
    }

    pub(super) fn question_cursor(&self, size: TerminalSize) -> Option<(usize, usize)> {
        let card = self.selected_approval()?;
        let draft = self.questions.drafts.get(&card.id)?;
        question_card::cursor(size, &draft.editor, self.question_controls(card))
    }

    fn question_controls(&self, card: &Approval) -> QuestionControls {
        QuestionControls {
            claude: self.task_backend() == Some(Backend::Claude),
            accept: card.choices.contains(&Decision::Once),
            session: card.choices.contains(&Decision::Session),
            decline: card.choices.contains(&Decision::Deny),
            cancel: card.choices.contains(&Decision::Cancel),
            blocking: (self.task_backend() == Some(Backend::Codex) && card.kind == "question")
                .then_some(card.blocking),
        }
    }
}
