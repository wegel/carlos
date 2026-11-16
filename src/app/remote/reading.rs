//! Keep visible source interests and history cursors separate from authoritative session facts.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::EntryId;

use super::transport::Transport;
use super::Device;
use crate::app::transcript_body::formatter::{Facts, View};
use crate::app::transcript_input::RenderInput;

struct HistoryRead {
    id: RequestId,
    before: EntryId,
}

#[derive(Default)]
pub(super) struct ReadingState {
    pub anchor: Option<(EntryId, usize)>,
    pub wanted: Vec<EntryId>,
    pub visible: Vec<EntryId>,
    pub formats: Vec<(EntryId, View)>,
    pub older_requested: bool,
    history: Option<HistoryRead>,
}

impl ReadingState {
    pub fn disconnected(&mut self) {
        self.history = None;
        self.older_requested = false;
    }

    pub fn received(&mut self, incoming: &Incoming) {
        if let Incoming::Failure { id: Some(id), .. } = incoming {
            if self.history.as_ref().is_some_and(|read| read.id == *id) {
                self.history = None;
            }
        }
    }
}

impl Device {
    /// Capture the address while the drawn index and current session still agree.
    pub(super) fn remember_anchor(&mut self) {
        self.reading.anchor = if self.view.viewport.auto_follow_bottom {
            None
        } else {
            self.view
                .render_cache
                .message_anchor(self.view.viewport.scroll_top)
                .and_then(|(index, row)| {
                    Some((
                        self.client
                            .session(&self.session)?
                            .entry_at(index)?
                            .id
                            .clone(),
                        row,
                    ))
                })
        };
    }

    /// Resolve only the current viewport into stable source addresses.
    pub(super) fn collect_visible(&mut self, height: usize) {
        self.reading.formats.clear();
        let cache = &self.view.render_cache;
        let first = self.view.viewport.scroll_top;
        let last =
            (first + height.saturating_sub(1)).min(cache.rendered_line_count().saturating_sub(1));
        if height == 0 || cache.entry_count() == 0 {
            return;
        }
        let Some((start, _)) = cache.message_anchor(first) else {
            return;
        };
        let end = cache.message_anchor(last).map_or(start, |(index, _)| index);
        if let Some(replica) = self.client.session(&self.session) {
            self.reading.visible.clear();
            self.reading.visible.extend(
                (start..=end)
                    .filter_map(|index| replica.entry_at(index).map(|entry| entry.id.clone())),
            );
            self.reading.wanted.extend(
                (start..=end)
                    .filter(|index| cache.needs_source(*index, first, last))
                    .filter_map(|index| replica.entry_at(index).map(|entry| entry.id.clone())),
            );
            self.reading
                .formats
                .extend(self.reading.wanted.iter().filter_map(|id| {
                    let index = replica.entry_position(id)?;
                    let entry = replica.entry_at(index)?;
                    let (format, previous, offset) = cache.format_window(index, first)?;
                    Some((
                        id.clone(),
                        View {
                            target: Default::default(),
                            facts: Facts::from_input(RenderInput::client_facts(entry)),
                            format,
                            previous,
                            first: offset,
                            length: height,
                        },
                    ))
                }));
        }
    }

    /// Write reads after drawing, without performing socket I/O inside the paint callback.
    pub(super) fn request_visible(&mut self, link: &mut Transport) -> Result<()> {
        if !self.client.session_is_ready(link.connection, &self.session) {
            return Ok(());
        }
        self.documents
            .prune(&mut self.client, link, &self.session)?;
        let mut wanted = self.reading.wanted.clone();
        if let Some(id) = self
            .documents
            .selection
            .as_ref()
            .and_then(|selection| selection.entry())
        {
            wanted.retain(|entry| entry != id);
            wanted.insert(0, id.clone());
        }
        if let Some(copy) = &self.documents.copy {
            wanted.retain(|id| id != &copy.entry);
            wanted.insert(0, copy.entry.clone());
        }
        if let Err(error) = self.documents.request_visible(
            &mut self.client,
            link,
            &self.session,
            &wanted,
            &self.reading.visible,
        ) {
            self.view.status_hint = Some(error.to_string());
        }
        self.request_history(link)?;
        link.flush()?;
        Ok(())
    }

    pub(super) fn settle_history(&mut self) {
        if self.reading.history.as_ref().is_some_and(|read| {
            self.client.session(&self.session).is_none_or(|replica| {
                !replica.has_older()
                    || replica
                        .entry_at(0)
                        .is_none_or(|entry| entry.id != read.before)
            })
        }) {
            self.reading.history = None;
        }
    }

    fn request_history(&mut self, link: &mut Transport) -> Result<()> {
        if !std::mem::take(&mut self.reading.older_requested) || self.reading.history.is_some() {
            return Ok(());
        }
        let Some(replica) = self
            .client
            .session(&self.session)
            .filter(|replica| replica.has_older())
        else {
            return Ok(());
        };
        let Some(before) = replica.entry_at(0).map(|entry| entry.id.clone()) else {
            return Ok(());
        };
        let outgoing = self.client.read(
            link.connection,
            Command::History {
                session: self.session.clone(),
                before: before.clone(),
                limit: 100,
            },
        )?;
        self.reading.history = Some(HistoryRead {
            id: outgoing.id(),
            before,
        });
        link.send(outgoing)?;
        Ok(())
    }
}
