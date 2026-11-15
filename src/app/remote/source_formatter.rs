//! Drive accepted source transfers and pause watches while the formatter queue is full.

use anyhow::Result;
use carlos_proto::source::INLINE_SOURCE_BYTES;

use super::formatting::Update;
use super::transport::Transport;
use super::Device;
use crate::app::transcript_body::formatter::{Facts, Transfer, View};
use crate::app::transcript_input::RenderInput;

impl Device {
    /// Keep a formatter error local to its source, preserving the socket and composer.
    pub(super) fn source_tick(&mut self, mut link: Option<&mut Transport>) -> bool {
        match self.drive_source_formatter(link.as_deref_mut()) {
            Ok(changed) => changed,
            Err(error) => {
                let entry = self
                    .documents
                    .formatting
                    .token()
                    .map(|token| token.entry().clone());
                self.documents.copy = None;
                self.documents.selection = None;
                self.documents.formatting.end_selection();
                if let Some(id) = entry {
                    if let Some(token) = self.documents.fail(&id) {
                        let effects = self.client.close_body(&token);
                        if let Some(link) = link {
                            for outgoing in effects.outgoing {
                                let _ = link.send(outgoing);
                            }
                        }
                    }
                    self.dirty_entry(&id);
                }
                self.view.status_hint = Some(error.to_string());
                true
            }
        }
    }

    /// Finish accepted work even offline; only watch writes require a ready connection.
    pub(super) fn drive_source_formatter(
        &mut self,
        mut link: Option<&mut Transport>,
    ) -> Result<bool> {
        let mut changed = false;
        for update in self.documents.formatting.poll()? {
            match update {
                Update::Rows(entry) => self.dirty_entry(&entry),
                Update::Copied { copy, backend } => {
                    if !self.selection_copied(copy, backend) {
                        self.copied_original(copy, backend);
                    }
                }
                Update::Selected(copy) => self.selection_part_copied(copy),
                Update::CopyFailed(error) => {
                    self.documents.selection = None;
                    self.documents.copy = None;
                    self.documents.formatting.end_selection();
                    self.view.status_hint = Some(format!("Cannot copy selected text: {error:?}"));
                }
            }
            changed = true;
        }
        changed |= self.cancel_unavailable_copy();
        self.drive_selection_copy()?;
        self.choose_source_formatter(link.as_deref_mut())?;
        self.drive_original_copy()?;
        let Some(token) = self.documents.formatting.token().cloned() else {
            if let Some(link) =
                link.filter(|link| self.client.session_is_ready(link.connection, &self.session))
            {
                self.documents
                    .drive_reads(&mut self.client, link, None, true)?;
            }
            return Ok(changed);
        };
        let state = self.documents.formatting.feed(self.client.bodies())?;
        let Some(link) =
            link.filter(|link| self.client.session_is_ready(link.connection, &self.session))
        else {
            return Ok(changed);
        };
        let accepting = (state != Transfer::Busy
            || !self
                .documents
                .formatting
                .has_unadmitted_bytes(self.client.bodies()))
            && !self.documents.formatting.selection_ready();
        self.documents
            .drive_reads(&mut self.client, link, Some(&token), accepting)?;
        Ok(changed)
    }

    fn formatter_candidate(&mut self) -> Option<(carlos_proto::ids::EntryId, View)> {
        if let Some(selection) = &self.documents.selection {
            selection.view()
        } else if self.documents.copy.is_some() {
            self.copy_view()
        } else {
            self.reading
                .formats
                .iter()
                .find(|(id, _)| {
                    self.documents
                        .token(id)
                        .and_then(|token| self.client.bodies().source(token))
                        .and_then(|window| window.descriptor())
                        .is_some_and(|source| source.bytes.get() > INLINE_SOURCE_BYTES as u64)
                })
                .cloned()
        }
    }

    fn choose_source_formatter(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if let Some(token) = self.documents.formatting.token().cloned() {
            if self.documents.token(token.entry()) != Some(&token)
                || self.client.bodies().source(&token).is_none()
            {
                self.documents.formatting.remove(token.entry());
            }
        }
        let Some((id, requested)) = self.formatter_candidate() else {
            return Ok(());
        };
        let Some(replica) = self.client.session(&self.session) else {
            return Ok(());
        };
        let Some(entry) = replica.entry(&id) else {
            return Ok(());
        };
        let Some(token) = self.documents.token(&id).cloned() else {
            return Ok(());
        };
        let Some(source) = self
            .client
            .bodies()
            .source(&token)
            .and_then(|body| body.descriptor())
        else {
            return Ok(());
        };
        if self.documents.formatting.token() != Some(&token)
            && self
                .client
                .bodies()
                .source(&token)
                .is_some_and(|body| body.start().get() > 0)
        {
            if let Some(link) =
                link.filter(|link| self.client.session_is_ready(link.connection, &self.session))
            {
                self.documents
                    .restart(&mut self.client, link, &self.session, &id)?;
            }
            return Ok(());
        }
        let input = RenderInput::client_facts(entry);
        let requested = match requested.target {
            crate::app::transcript_body::formatter::Target::Selection(_) => requested,
            _ => View {
                facts: Facts::from_input(input),
                ..requested
            },
        };
        self.bind_formatter(link, &token, source.revision, requested)
    }

    fn bind_formatter(
        &mut self,
        link: Option<&mut Transport>,
        token: &carlos_client::BodyToken,
        revision: carlos_proto::output::OutputRevision,
        requested: View,
    ) -> Result<()> {
        if let Err(error) = self
            .documents
            .formatting
            .request(token, revision, requested)
        {
            if let Some(token) = self.documents.fail(token.entry()) {
                let effects = self.client.close_body(&token);
                if let Some(link) = link {
                    for outgoing in effects.outgoing {
                        link.send(outgoing)?;
                    }
                }
            }
            return Err(error);
        }
        Ok(())
    }
}
