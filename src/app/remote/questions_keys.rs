//! Route callback keys to their original private editor, never to the ordinary composer.

use anyhow::{ensure, Result};
use carlos_core::session::entry_id::EntryId;
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{Approval, Backend, Decision};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use super::questions_answers;
use super::transport::Transport;
use super::Device;
use crate::app::claude_question_state::can_edit_question;
use crate::app::claude_question_view::default_no_hint;
use crate::app::question_editor;

impl Device {
    pub(super) fn question_key(
        &mut self,
        mut link: Option<&mut Transport>,
        key: KeyEvent,
    ) -> Result<bool> {
        if key.code == KeyCode::F(4) {
            self.next_question();
            return Ok(true);
        }
        let Some(card) = self.selected_approval().cloned() else {
            return Ok(false);
        };
        if self.question_control_key(link.as_deref_mut(), key, &card)? {
            return Ok(true);
        }
        if card.input.is_none() {
            let handled = self.question_decision_key(link, key, &card)?;
            return Ok(handled || self.task_backend() != Some(Backend::Grok));
        }
        self.question_editor_key(link, key, &card)?;
        Ok(true)
    }

    fn question_control_key(
        &mut self,
        mut link: Option<&mut Transport>,
        key: KeyEvent,
        card: &Approval,
    ) -> Result<bool> {
        if key.code == KeyCode::F(10) {
            self.interrupt_question(link.as_deref_mut(), card)?;
            return Ok(true);
        }
        if key.code == KeyCode::Esc && !card.blocking {
            self.questions.shown = false;
            return Ok(true);
        }
        if key.code == KeyCode::Char('y') && key.modifiers == KeyModifiers::CONTROL {
            self.copy_question_url(card)?;
            return Ok(true);
        }
        let empty = self
            .questions
            .drafts
            .get(&card.id)
            .is_some_and(|draft| draft.editor.fields.is_empty());
        if empty && decision(key, true).is_some() {
            return self.question_decision_key(link.as_deref_mut(), key, card);
        }
        if let Some(decision) = decision(key, false) {
            self.send_question_answer(
                link,
                card.id.clone(),
                ApprovalAnswer::Decision { decision },
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    fn interrupt_question(&mut self, link: Option<&mut Transport>, card: &Approval) -> Result<()> {
        if self.task_backend() == Some(Backend::Claude) {
            let link =
                link.ok_or_else(|| anyhow::anyhow!("Disconnected; your answer draft is kept"))?;
            ensure!(
                self.question_facts_are_current()
                    && self
                        .client
                        .session_has_current_snapshot(link.connection, &self.session),
                "Wait for accepted session facts before interrupting; your answer draft is kept"
            );
            if self
                .client
                .session(&self.session)
                .is_some_and(|replica| replica.info().current_turn.is_some())
            {
                self.interrupt(link)?;
            } else {
                self.send_question_answer(
                    Some(link),
                    card.id.clone(),
                    ApprovalAnswer::InterruptQuery,
                )?;
            }
        } else {
            self.send_question_answer(link, card.id.clone(), ApprovalAnswer::InterruptQuery)?;
        }
        Ok(())
    }

    fn question_editor_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
        card: &Approval,
    ) -> Result<()> {
        let input = card.input.as_ref().expect("structured question");
        if !self.questions.size.is_some_and(can_edit_question) {
            return Ok(());
        }
        if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL {
            let draft = self.questions.drafts.get_mut(&card.id).ok_or_else(|| {
                anyhow::anyhow!("Original question changed; F4 opens a fresh editor")
            })?;
            if card.default_to_no && !draft.confirm {
                draft.confirm = true;
                draft.editor.notice =
                    Some(default_no_hint(!draft.editor.fields.is_empty(), true).into());
                return Ok(());
            }
            let answer =
                questions_answers::answer(input, &draft.editor).map_err(anyhow::Error::msg)?;
            self.send_question_answer(link, card.id.clone(), answer)?;
        } else if let Some(draft) = self.questions.drafts.get_mut(&card.id) {
            if draft.editor.review
                && !matches!(
                    key.code,
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::PageUp | KeyCode::PageDown
                )
            {
                return Ok(());
            }
            draft.confirm = false;
            if card.default_to_no {
                draft.editor.notice =
                    Some(default_no_hint(!draft.editor.fields.is_empty(), false).into())
            }
            question_editor::key(&mut draft.editor, key);
        }
        Ok(())
    }

    fn question_decision_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
        card: &Approval,
    ) -> Result<bool> {
        let Some(decision) = decision(key, true) else {
            return Ok(false);
        };
        if !card.choices.contains(&decision) {
            return Ok(true);
        }
        if matches!(decision, Decision::Once | Decision::Session) {
            if let Some(error) = self
                .questions
                .drafts
                .get(&card.id)
                .and_then(|draft| draft.editor.unsupported.as_ref())
            {
                anyhow::bail!("Cannot accept this original request: {error}");
            }
        }
        if decision == Decision::Once && !self.questions.size.is_some_and(can_edit_question) {
            return Ok(true);
        }
        if decision == Decision::Once
            && card.default_to_no
            && self.view.confirm_allow != Some(EntryId::from_wire(&card.id))
        {
            self.view.confirm_allow = Some(EntryId::from_wire(&card.id));
            if let Some(draft) = self.questions.drafts.get_mut(&card.id) {
                draft.confirm = true;
                draft.editor.notice = Some(default_no_hint(false, true).into());
            }
            self.view.status_hint = Some("Press y again to allow this request".into());
            return Ok(true);
        }
        self.send_question_answer(link, card.id.clone(), ApprovalAnswer::Decision { decision })?;
        self.view.confirm_allow = None;
        Ok(true)
    }

    pub(super) fn question_paste(&mut self, text: &str) -> bool {
        let Some(card) = self.selected_approval() else {
            return false;
        };
        let id = card.id.clone();
        if card.input.is_none() && self.task_backend() == Some(Backend::Grok) {
            return false;
        }
        let editable = self.questions.size.is_some_and(can_edit_question);
        if let Some(draft) = self.questions.drafts.get_mut(&id) {
            if editable && !draft.editor.review {
                if let Some(field) = draft.editor.fields.get_mut(draft.editor.current) {
                    field.paste(text);
                }
                draft.confirm = false;
            }
        }
        true
    }

    pub(super) fn question_mouse(&mut self, event: MouseEvent) {
        let Some(draft) = self
            .questions
            .focus
            .as_ref()
            .and_then(|id| self.questions.drafts.get_mut(id))
        else {
            return;
        };
        let viewport = &draft.editor.viewport;
        let scroll = match event.kind {
            MouseEventKind::ScrollUp => viewport.scroll.get().saturating_sub(3),
            MouseEventKind::ScrollDown => viewport
                .scroll
                .get()
                .saturating_add(3)
                .min(viewport.max_scroll.get()),
            _ => return,
        };
        viewport.scroll.set(scroll);
        viewport.follow_focus.set(false);
    }

    pub(super) fn question_error(&mut self, message: String) {
        if let Some(draft) = self
            .questions
            .focus
            .as_ref()
            .and_then(|id| self.questions.drafts.get_mut(id))
        {
            draft.editor.error = Some(message.clone());
            draft.editor.viewport.scroll.set(0);
        }
        self.view.status_hint = Some(message);
    }
}

fn decision(key: KeyEvent, empty: bool) -> Option<Decision> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('n'), KeyModifiers::ALT) => Some(Decision::Deny),
        (KeyCode::Char('c'), KeyModifiers::ALT) | (KeyCode::F(10), _) => Some(Decision::Cancel),
        (KeyCode::Char('y' | 'a'), KeyModifiers::NONE) if empty => Some(Decision::Once),
        (KeyCode::Char('s'), KeyModifiers::NONE) if empty => Some(Decision::Session),
        (KeyCode::Char('n'), KeyModifiers::NONE) if empty => Some(Decision::Deny),
        (KeyCode::Char('c'), KeyModifiers::NONE) if empty => Some(Decision::Cancel),
        _ => None,
    }
}
