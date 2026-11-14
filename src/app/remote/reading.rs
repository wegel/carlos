//! Keep visible source interests and history cursors separate from authoritative session facts.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::events::Notification;
use carlos_proto::ids::{EntryId, SessionId};

use super::transport::Transport;
use super::Device;

struct HistoryRead {
    id: RequestId,
    before: EntryId,
}

#[derive(Default)]
pub(super) struct ReadingState {
    pub anchor: Option<(EntryId, usize)>,
    pub wanted: Vec<EntryId>,
    pub visible: Vec<EntryId>,
    pub loading: bool,
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
        let cache = &self.view.render_cache;
        let first = self.view.viewport.scroll_top;
        let last =
            (first + height.saturating_sub(1)).min(cache.rendered_line_count().saturating_sub(1));
        self.reading.loading = cache.has_loading_rows(first, last);
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
                    .filter(|index| cache.needs_source(*index))
                    .filter_map(|index| replica.entry_at(index).map(|entry| entry.id.clone())),
            );
        }
    }

    /// Write reads after drawing, without performing socket I/O inside the paint callback.
    pub(super) fn request_visible(&mut self, link: &mut Transport) -> Result<()> {
        if !self.client.session_is_ready(link.connection, &self.session) {
            return Ok(());
        }
        self.documents
            .prune(&mut self.client, link, &self.session)?;
        if let Err(error) = self.documents.request_visible(
            &mut self.client,
            link,
            &self.session,
            &self.reading.wanted,
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

    /// Release unused body data before a live source chunk reaches the checked client cache.
    pub(super) fn reserve_source_growth(
        &mut self,
        link: &mut Transport,
        incoming: &Incoming,
    ) -> Result<()> {
        let Some((entry, bytes)) = announced_source(incoming, &self.session) else {
            return Ok(());
        };
        if self.reading.visible.contains(&entry) {
            self.documents.reserve_growth(
                &mut self.client,
                link,
                &self.session,
                &entry,
                bytes,
                &self.reading.visible,
            )?;
        }
        Ok(())
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

// These hints can release only unused cache data. Client::receive still checks the reply's
// request/token, revision, offset and bytes before accepting any of its original text.
fn announced_source(incoming: &Incoming, session: &SessionId) -> Option<(EntryId, u64)> {
    match incoming {
        Incoming::Notification(notification) => match notification.as_ref() {
            Notification::Source(update) if &update.session == session => {
                Some((update.chunk.entry.clone(), update.chunk.source.bytes.get()))
            }
            _ => None,
        },
        Incoming::Success { result, .. } => Some((
            result.get("entry")?.as_str()?.parse().ok()?,
            result.get("source")?.get("bytes")?.as_str()?.parse().ok()?,
        )),
        _ => None,
    }
}
