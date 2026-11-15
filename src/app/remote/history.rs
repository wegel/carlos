//! Keep each terminal's prompt cursor and original composer separate from shared session facts.

use std::collections::VecDeque;

use anyhow::{bail, Result};
use carlos_client::{Effects, ReadReply, Replica};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::prompts::{PromptDirection, PromptSelection, PromptSource};
use carlos_proto::session::Backend;
use carlos_proto::submissions::ClientOperation;

use super::reading::ReadingState;
use super::transport::Transport;
use super::Device;
use crate::app::input::make_input_area;
use crate::app::render_cache_state::RenderCacheState;
use crate::app::viewport_state::ViewportState;

struct PromptLoad {
    selection: PromptSelection,
    text: String,
}

#[derive(Default)]
pub(super) struct History {
    pub selection: Option<PromptSelection>,
    pub pending: Option<RequestId>,
    load: Option<PromptLoad>,
    pub navigation_draft: Option<String>,
    pub rewind_draft: Option<String>,
    pub operation: Option<ClientOperation>,
    pub failed: bool,
    pub recoveries: VecDeque<ClientOperation>,
    pub moves: VecDeque<bool>,
    align_pending: bool,
}

impl History {
    pub(super) fn cancel_read(&mut self) {
        self.pending = None;
        self.load = None;
        self.moves.clear();
    }

    pub(super) fn reset_navigation(&mut self) {
        self.cancel_read();
        self.selection = None;
        self.navigation_draft = None;
        self.align_pending = false;
    }

    pub(super) fn align_view(
        &mut self,
        replica: Option<&Replica>,
        cache: &RenderCacheState,
        viewport: &mut ViewportState,
        reading: &mut ReadingState,
        height: usize,
    ) {
        if !self.align_pending || self.rewind_draft.is_none() {
            return;
        }
        let entry = self
            .selection
            .as_ref()
            .and_then(|selection| selection.record.as_ref())
            .and_then(|record| record.entry.as_ref());
        let Some(entry) = entry else {
            self.align_pending = false;
            return;
        };
        if let Some(position) = replica.and_then(|replica| replica.entry_position(entry)) {
            if let Some(row) = cache.block_start(position) {
                viewport.scroll_top = row.saturating_sub(height.saturating_sub(1));
                viewport.auto_follow_bottom = false;
                reading.anchor = None;
                self.align_pending = false;
            }
        } else if replica.is_some_and(Replica::has_older) {
            reading.older_requested = true;
            viewport.scroll_top = 0;
            reading.anchor = None;
        } else {
            self.align_pending = false;
        }
    }
}

impl Device {
    pub(super) fn select_prompt(
        &mut self,
        link: &mut Transport,
        source: PromptSource,
        direction: PromptDirection,
    ) -> Result<()> {
        if self.history.pending.is_some() {
            return Ok(());
        }
        if !self.client.session_is_ready(link.connection, &self.session) {
            bail!("Waiting for current ready history; draft kept");
        }
        let anchor = if direction == PromptDirection::Latest {
            None
        } else {
            self.history
                .selection
                .as_ref()
                .and_then(|selection| selection.record.as_ref())
                .map(|record| record.id.clone())
        };
        if self.history.navigation_draft.is_none() {
            self.history.navigation_draft = Some(self.view.input.lines().join("\n"));
        }
        let outgoing = self.client.read(
            link.connection,
            Command::PromptSelect {
                session: self.session.clone(),
                source,
                anchor,
                direction,
            },
        )?;
        self.history.pending = Some(outgoing.id());
        self.view.status_hint = Some("Reading saved prompt…".into());
        link.send(outgoing)
    }

    pub(super) fn apply_prompt_reads(
        &mut self,
        link: &mut Transport,
        effects: &mut Effects,
    ) -> Result<bool> {
        let mut changed = false;
        for read in std::mem::take(&mut effects.replies) {
            if self.history.pending.is_none() || read.id != self.history.pending {
                effects.replies.push(read);
                continue;
            }
            self.history.pending = None;
            match read.reply {
                ReadReply::PromptSelection(selection) => self.prompt_selected(link, selection)?,
                ReadReply::PromptText(chunk) => self.prompt_text_received(link, chunk)?,
                _ => bail!("prompt read returned another reply kind"),
            }
            changed = true;
        }
        if effects
            .failures
            .iter()
            .any(|failure| failure.id.is_some() && failure.id == self.history.pending)
        {
            self.history.cancel_read();
            changed = true;
        }
        Ok(changed)
    }

    fn prompt_selected(&mut self, link: &mut Transport, selection: PromptSelection) -> Result<()> {
        if selection.record.is_none() {
            self.history.selection = Some(selection);
            let draft = self.history.navigation_draft.take().unwrap_or_default();
            self.set_composer(&draft);
            self.view.status_hint = self.history.rewind_draft.as_ref().map(|_| {
                if self
                    .client
                    .session(&self.session)
                    .is_some_and(|r| r.info().backend == Backend::Claude)
                {
                    "F4: older unknown prompts · no human prompt selected"
                } else {
                    "No earlier prompt selected · Up chooses history"
                }
                .into()
            });
            return self.continue_prompt_navigation(link);
        }
        self.history.load = Some(PromptLoad {
            selection,
            text: String::new(),
        });
        self.request_prompt_piece(link)
    }

    fn request_prompt_piece(&mut self, link: &mut Transport) -> Result<()> {
        let load = self.history.load.as_ref().expect("selected prompt text");
        let record = load
            .selection
            .record
            .as_ref()
            .expect("selected prompt record");
        let outgoing = self.client.read(
            link.connection,
            Command::PromptText {
                session: self.session.clone(),
                prompt: record.id.clone(),
                offset: ByteOffset::new(load.text.len() as u64),
                limit: Some(65536),
            },
        )?;
        self.history.pending = Some(outgoing.id());
        link.send(outgoing)
    }

    fn prompt_text_received(
        &mut self,
        link: &mut Transport,
        chunk: carlos_proto::prompts::PromptText,
    ) -> Result<()> {
        let load = self.history.load.as_mut().expect("selected prompt text");
        let record = load
            .selection
            .record
            .as_ref()
            .expect("selected prompt record");
        if chunk.prompt != record.id
            || chunk.total != record.bytes
            || chunk.offset.get() != load.text.len() as u64
        {
            self.history.cancel_read();
            bail!("selected prompt changed while reading; draft kept");
        }
        load.text.push_str(&chunk.text);
        if chunk.next.is_some() {
            return self.request_prompt_piece(link);
        }
        let load = self.history.load.take().expect("complete prompt");
        let source = load.selection.source;
        self.set_composer(&load.text);
        self.history.selection = Some(load.selection);
        self.view.status_hint = if self.history.rewind_draft.is_some() {
            Some(if source == PromptSource::Unknown {
                "Source unknown: Enter forks before this prompt; F4 human prompts; Esc restores draft"
            } else {
                "Edit human prompt; F4 selects older prompts with unknown source"
            }.into())
        } else {
            None
        };
        self.history.align_pending = true;
        self.continue_prompt_navigation(link)
    }

    fn continue_prompt_navigation(&mut self, link: &mut Transport) -> Result<()> {
        if let Some(older) = self.history.moves.pop_front() {
            self.navigate_prompt(Some(link), older)?;
        }
        Ok(())
    }

    pub(super) fn set_composer(&mut self, text: &str) {
        self.view.input = make_input_area();
        self.view.input.insert_str(text);
    }
}
