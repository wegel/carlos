//! Keep each terminal's callback focus and answer editors bound to exact live card descriptions.

use std::collections::{HashMap, HashSet};

use carlos_proto::ids::EntryId;
use carlos_proto::session::{Approval, Availability};
use carlos_proto::submissions::ClientOperation;

use super::Device;
use crate::app::claude_question_state::NativeDraft;
use crate::app::claude_question_view::default_no_hint;
use crate::app::view_models::TerminalSize;

pub(super) struct AnswerDraft {
    pub(super) editor: NativeDraft,
    pub(super) fingerprint: [u8; 32],
    pub(super) confirm: bool,
}

pub(super) struct AnswerTarget {
    pub(super) card: EntryId,
    pub(super) fingerprint: [u8; 32],
    pub(super) finished: bool,
    pub(super) interrupt: bool,
}

#[derive(Default)]
pub(super) struct Questions {
    pub(super) url: Option<super::questions_url::UrlCopy>,
    pub(super) size: Option<TerminalSize>,
    pub(super) drafts: HashMap<EntryId, AnswerDraft>,
    pub(super) operations: HashMap<ClientOperation, AnswerTarget>,
    receipt: Option<(EntryId, String)>,
    retired: HashSet<EntryId>,
    pub(super) focus: Option<EntryId>,
    pub(super) shown: bool,
    pub(super) auto_focus: bool,
    successor: Option<EntryId>,
}

impl Questions {
    pub(super) fn clear_retry_hint(&self, card: &EntryId, hint: &mut Option<String>) {
        let error = self
            .drafts
            .get(card)
            .and_then(|draft| draft.editor.error.as_ref());
        if error.is_some() && hint.as_ref() == error {
            *hint = None;
        }
    }

    pub(super) fn accept_receipt(
        &mut self,
        card: &EntryId,
        message: String,
        cards: &[Approval],
        hint: &mut Option<String>,
    ) {
        self.retire_receipt(cards, hint);
        if cards.iter().any(|live| &live.id == card) {
            *hint = Some(message.clone());
            self.receipt = Some((card.clone(), message));
        }
    }

    fn retire_receipt(&mut self, cards: &[Approval], hint: &mut Option<String>) {
        let Some((id, message)) = &self.receipt else {
            return;
        };
        if cards.iter().any(|card| &card.id == id) {
            return;
        }
        if hint.as_ref() == Some(message) {
            *hint = None;
        }
        self.receipt = None;
    }

    pub(super) fn synchronize(&mut self, cards: &[Approval]) {
        let successor = self.shown.then(|| self.successor.clone()).flatten();
        self.operations.retain(|_, target| {
            !target.finished || cards.iter().any(|card| card.id == target.card)
        });
        self.drafts
            .retain(|id, _| cards.iter().any(|card| &card.id == id));
        self.retired
            .retain(|id| cards.iter().any(|card| &card.id == id));
        for card in cards {
            self.ensure_draft(card);
        }
        if !cards
            .iter()
            .any(|card| Some(&card.id) == self.focus.as_ref())
        {
            self.focus = None;
            self.shown = false;
            if let Some(card) = cards.iter().find(|card| {
                Some(&card.id) == successor.as_ref() && !card.submitted && card.blocking
            }) {
                self.focus = Some(card.id.clone());
                self.shown = true;
                self.auto_focus = false;
            }
        }
        self.select_automatic(cards);
        self.remember_successor(cards);
    }

    fn remember_successor(&mut self, cards: &[Approval]) {
        self.successor = self.focus.as_ref().and_then(|focus| {
            let index = cards.iter().position(|card| &card.id == focus)?;
            cards
                .iter()
                .cycle()
                .skip(index + 1)
                .take(cards.len().saturating_sub(1))
                .find(|card| !card.submitted)
                .map(|card| card.id.clone())
        });
    }

    fn select_automatic(&mut self, cards: &[Approval]) {
        if !self.shown || self.auto_focus {
            let next = cards
                .iter()
                .filter(|card| !card.submitted && card.blocking && !self.retired.contains(&card.id))
                .find(|card| card.active)
                .or_else(|| {
                    cards.iter().find(|card| {
                        !card.submitted && card.blocking && !self.retired.contains(&card.id)
                    })
                });
            if let Some(card) = next {
                self.focus = Some(card.id.clone());
                self.shown = true;
                self.auto_focus = true;
            }
        }
    }

    fn ensure_draft(&mut self, card: &Approval) {
        let fingerprint = fingerprint(card);
        if self
            .drafts
            .get(&card.id)
            .is_some_and(|draft| draft.fingerprint != fingerprint)
        {
            self.drafts.remove(&card.id);
            self.retired.insert(card.id.clone());
            if self.focus.as_ref() == Some(&card.id) {
                self.focus = None;
                self.shown = false;
            }
        }
        if self.retired.contains(&card.id) {
            return;
        }
        let draft = self
            .drafts
            .entry(card.id.clone())
            .or_insert_with(|| AnswerDraft {
                editor: initial_editor(card),
                fingerprint,
                confirm: false,
            });
        if let Some(error) = &card.input_error {
            draft.editor.unsupported = Some(error.clone());
        }
    }

    pub(super) fn next(&mut self, cards: &[Approval]) {
        self.auto_focus = false;
        let live: Vec<_> = cards.iter().filter(|card| !card.submitted).collect();
        if live.is_empty() {
            return;
        }
        let index = self
            .focus
            .as_ref()
            .and_then(|id| live.iter().position(|card| &card.id == id));
        let index = if self.shown {
            index.map_or(0, |index| (index + 1) % live.len())
        } else {
            index.unwrap_or(0)
        };
        self.focus = Some(live[index].id.clone());
        if self.retired.remove(&live[index].id) {
            self.drafts.insert(
                live[index].id.clone(),
                AnswerDraft {
                    editor: initial_editor(live[index]),
                    fingerprint: fingerprint(live[index]),
                    confirm: false,
                },
            );
        }
        if let Some(draft) = self.drafts.get_mut(&live[index].id) {
            if draft.confirm && live[index].default_to_no {
                draft.editor.notice =
                    Some(default_no_hint(!draft.editor.fields.is_empty(), false).into());
            }
            draft.confirm = false;
        }
        self.shown = true;
        self.remember_successor(cards);
    }
}

fn initial_editor(card: &Approval) -> NativeDraft {
    let mut editor = NativeDraft::from_approval(card.input.as_ref());
    if let Some(error) = &card.input_error {
        editor.unsupported = Some(error.clone());
    }
    if card.default_to_no {
        editor.notice = Some(default_no_hint(!editor.fields.is_empty(), false).into());
    }
    editor
}

impl Device {
    pub(super) fn question_effects(&mut self, effects: &carlos_client::Effects) {
        let replaced = matches!(
            effects.welcome,
            Some(carlos_client::Welcome::Replaced { .. })
        );
        if replaced
            || effects.sessions.iter().any(|update| {
                update.session == self.session && (update.changes.reset || update.changes.approvals)
            })
        {
            self.sync_questions(replaced);
        }
    }

    pub(super) fn sync_questions(&mut self, replaced: bool) {
        if replaced {
            self.questions
                .retire_receipt(&[], &mut self.view.status_hint);
            self.questions.drafts.clear();
            self.questions.retired.clear();
            self.questions.focus = None;
            self.questions.shown = false;
        }
        if !self.question_facts_are_current() {
            return;
        }
        if let Some(replica) = self.client.session(&self.session) {
            self.questions
                .retire_receipt(replica.approvals(), &mut self.view.status_hint);
            self.questions.synchronize(replica.approvals());
        }
    }

    pub(super) fn next_question(&mut self) {
        self.view.confirm_allow = None;
        if let Some(replica) = self.client.session(&self.session) {
            self.questions.next(replica.approvals());
        }
    }

    pub(super) fn selected_approval(&self) -> Option<&Approval> {
        if !self.questions.shown {
            return None;
        }
        self.client
            .session(&self.session)?
            .approvals()
            .iter()
            .find(|card| Some(&card.id) == self.questions.focus.as_ref())
    }

    pub(super) fn question_input_is_secret(&self) -> bool {
        self.selected_approval()
            .and_then(|card| self.questions.drafts.get(&card.id))
            .is_some_and(|draft| {
                if draft.editor.review {
                    draft.editor.fields.iter().any(|field| field.spec.secret())
                } else {
                    draft
                        .editor
                        .fields
                        .get(draft.editor.current)
                        .is_some_and(|field| field.spec.secret())
                }
            })
    }

    pub(super) fn question_facts_are_current(&self) -> bool {
        self.client.current_session_has_snapshot(&self.session)
            && self.client.session(&self.session).is_some_and(|replica| {
                matches!(
                    replica.info().availability,
                    Availability::Ready | Availability::Starting | Availability::Restoring
                )
            })
    }
}

pub(super) fn fingerprint(card: &Approval) -> [u8; 32] {
    let bytes = serde_json::to_vec(&(
        &card.kind,
        &card.input,
        &card.choices,
        card.default_to_no,
        card.blocking,
    ))
    .expect("typed callback description");
    *blake3::hash(&bytes).as_bytes()
}

#[cfg(test)]
#[path = "questions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "questions_editor_tests.rs"]
mod editor_tests;

#[cfg(test)]
#[path = "questions_ownership_tests.rs"]
mod ownership_tests;
