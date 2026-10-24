//! Lazy render cache that counts lines eagerly and materializes blocks on demand.

use super::models::{Message, RenderedLine, Role};
use super::tool_output_state::compact_tool_message;
use super::transcript_render::{
    build_rendered_block_for_message, count_rendered_block_for_message_cached, RenderCountCache,
};

// --- Cache State ---

pub(super) struct RenderCacheState {
    pub(super) rendered_message_blocks: Vec<Option<Vec<RenderedLine>>>,
    pub(super) rendered_block_line_counts: Vec<usize>,
    pub(super) rendered_block_offsets: Vec<usize>,
    pub(super) rendered_total_lines: usize,
    pub(super) rendered_width: usize,
    pub(super) rendered_compact: bool,
    compact_messages: Vec<Option<Box<Message>>>,
    pub(super) rendered_hidden_user_message_idx: Option<usize>,
    pub(super) transcript_dirty_from: Option<usize>,
}

impl RenderCacheState {
    pub(super) fn new() -> Self {
        Self {
            rendered_message_blocks: Vec::new(),
            rendered_block_line_counts: Vec::new(),
            rendered_block_offsets: Vec::new(),
            rendered_total_lines: 0,
            rendered_width: 0,
            rendered_compact: false,
            compact_messages: Vec::new(),
            rendered_hidden_user_message_idx: None,
            transcript_dirty_from: Some(0),
        }
    }

    // --- Cache Lifecycle ---
    pub(super) fn mark_transcript_dirty_from(&mut self, messages_len: usize, idx: usize) {
        let idx = idx.min(messages_len);
        self.transcript_dirty_from = Some(match self.transcript_dirty_from {
            Some(current) => current.min(idx),
            None => idx,
        });
    }

    // --- Count Pass ---
    pub(super) fn ensure_rendered_lines(
        &mut self,
        messages: &[Message],
        width: usize,
        hidden_user_message_idx: Option<usize>,
        compact: bool,
    ) {
        let rebuild_from = if self.rendered_width != width
            || self.rendered_compact != compact
            || self.rendered_hidden_user_message_idx != hidden_user_message_idx
        {
            Some(0)
        } else {
            self.transcript_dirty_from
        };

        let Some(dirty_from) = rebuild_from else {
            return;
        };

        self.truncate_rendered_blocks_to(dirty_from);
        self.rebuild_rendered_blocks_from(
            messages,
            width,
            hidden_user_message_idx,
            dirty_from,
            compact,
        );

        self.rendered_width = width;
        self.rendered_compact = compact;
        self.rendered_hidden_user_message_idx = hidden_user_message_idx;
        self.transcript_dirty_from = None;
    }

    fn truncate_rendered_blocks_to(&mut self, dirty_from: usize) {
        if dirty_from == 0 {
            self.rendered_message_blocks.clear();
            self.compact_messages.clear();
            self.rendered_block_line_counts.clear();
            self.rendered_block_offsets.clear();
            self.rendered_total_lines = 0;
            return;
        }
        let start_offset = self
            .rendered_block_offsets
            .get(dirty_from)
            .copied()
            .unwrap_or(self.rendered_total_lines);
        self.rendered_message_blocks.truncate(dirty_from);
        self.compact_messages.truncate(dirty_from);
        self.rendered_block_line_counts.truncate(dirty_from);
        self.rendered_block_offsets.truncate(dirty_from);
        self.rendered_total_lines = start_offset;
    }

    fn rebuild_rendered_blocks_from(
        &mut self,
        messages: &[Message],
        width: usize,
        hidden_user_message_idx: Option<usize>,
        dirty_from: usize,
        compact: bool,
    ) {
        let mut previous_visible_idx = if dirty_from == 0 {
            None
        } else {
            find_previous_visible_message_idx(messages, dirty_from, hidden_user_message_idx)
        };
        let mut count_cache = RenderCountCache::new();

        for idx in dirty_from..messages.len() {
            self.rendered_block_offsets.push(self.rendered_total_lines);
            self.compact_messages.push(None);

            let hidden = hidden_user_message_idx == Some(idx) && messages[idx].role == Role::User;
            if hidden {
                self.rendered_message_blocks.push(None);
                self.rendered_block_line_counts.push(0);
                continue;
            }

            let msg = &messages[idx];
            if msg.text.trim().is_empty() {
                self.rendered_message_blocks.push(None);
                self.rendered_block_line_counts.push(0);
                continue;
            }

            let previous_visible = previous_visible_idx.and_then(|prev_idx| messages.get(prev_idx));
            let (folded, line_count) =
                count_display_block(&mut count_cache, previous_visible, msg, width, compact);
            self.compact_messages[idx] = folded.map(Box::new);
            self.rendered_block_line_counts.push(line_count);
            if line_count == 0 {
                self.rendered_message_blocks.push(None);
                continue;
            }
            self.rendered_total_lines += line_count;
            self.rendered_message_blocks.push(None);
            previous_visible_idx = Some(idx);
        }
    }

    // --- Line Access ---
    pub(super) fn rendered_line_count(&self) -> usize {
        self.rendered_total_lines
    }

    pub(super) fn rendered_line_at(&self, idx: usize) -> Option<&RenderedLine> {
        if idx >= self.rendered_total_lines {
            return None;
        }
        let block_idx = self
            .rendered_block_offsets
            .partition_point(|&start| start <= idx)
            .checked_sub(1)?;
        let block_start = *self.rendered_block_offsets.get(block_idx)?;
        let block = self.rendered_message_blocks.get(block_idx)?.as_ref()?;
        block.get(idx - block_start)
    }

    /// Locate a message and row before a resize or view change changes row counts.
    pub(super) fn message_anchor(&self, line: usize) -> Option<(usize, usize)> {
        let index = self
            .rendered_block_offsets
            .partition_point(|&start| start <= line)
            .checked_sub(1)?;
        Some((
            index,
            line.saturating_sub(self.rendered_block_offsets[index]),
        ))
    }

    /// Keep the same message visible after rebuilding the row counts.
    pub(super) fn line_for_anchor(&self, (index, row): (usize, usize)) -> Option<usize> {
        let start = *self.rendered_block_offsets.get(index)?;
        let count = *self.rendered_block_line_counts.get(index)?;
        Some(start + row.min(count.saturating_sub(1)))
    }

    // --- Materialization ---
    pub(super) fn ensure_rendered_range_materialized(
        &mut self,
        messages: &[Message],
        start_idx: usize,
        end_idx: usize,
    ) {
        if start_idx > end_idx || self.rendered_total_lines == 0 {
            return;
        }

        let start_block = self
            .rendered_block_offsets
            .partition_point(|&start| start <= start_idx)
            .saturating_sub(1);
        let end_block = self
            .rendered_block_offsets
            .partition_point(|&start| start <= end_idx)
            .saturating_sub(1);

        for block_idx in start_block..=end_block {
            self.materialize_block(messages, block_idx);
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot_rendered_lines(&mut self, messages: &[Message]) -> Vec<RenderedLine> {
        if self.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(messages, 0, self.rendered_total_lines - 1);
        }
        let mut out = Vec::with_capacity(self.rendered_total_lines);
        for block in &self.rendered_message_blocks {
            if let Some(block) = block {
                out.extend(block.iter().cloned());
            }
        }
        out
    }

    fn materialize_block(&mut self, messages: &[Message], block_idx: usize) {
        if self
            .rendered_message_blocks
            .get(block_idx)
            .and_then(Option::as_ref)
            .is_some()
        {
            return;
        }
        if self
            .rendered_block_line_counts
            .get(block_idx)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return;
        }

        let Some(msg) = self
            .compact_messages
            .get(block_idx)
            .and_then(Option::as_deref)
            .or_else(|| messages.get(block_idx))
        else {
            return;
        };
        let previous_visible = find_previous_visible_message_idx(
            messages,
            block_idx,
            self.rendered_hidden_user_message_idx,
        )
        .and_then(|prev_idx| messages.get(prev_idx));
        let block = build_rendered_block_for_message(previous_visible, msg, self.rendered_width);
        debug_assert_eq!(
            block.len(),
            self.rendered_block_line_counts
                .get(block_idx)
                .copied()
                .unwrap_or(0)
        );
        if let Some(slot) = self.rendered_message_blocks.get_mut(block_idx) {
            *slot = Some(block);
        }
    }
}

// --- Visibility Lookup ---

/// Count the same full or folded message that the cache will draw.
pub(super) fn count_display_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<&Message>,
    message: &'a Message,
    width: usize,
    compact: bool,
) -> (Option<Message>, usize) {
    let full_count = count_rendered_block_for_message_cached(cache, previous, message, width);
    let rows = full_count.saturating_sub(usize::from(previous.is_some()));
    let folded = compact
        .then(|| compact_tool_message(message, rows, width))
        .flatten();
    let count = folded
        .as_ref()
        .map(|summary| {
            count_rendered_block_for_message_cached(
                &mut RenderCountCache::new(),
                previous,
                summary,
                width,
            )
        })
        .unwrap_or(full_count);
    (folded, count)
}

fn find_previous_visible_message_idx(
    messages: &[Message],
    start_idx: usize,
    hidden_user_message_idx: Option<usize>,
) -> Option<usize> {
    let mut idx = start_idx;
    while idx > 0 {
        idx -= 1;
        let msg = messages.get(idx)?;
        if hidden_user_message_idx == Some(idx) && msg.role == Role::User {
            continue;
        }
        if !msg.text.trim().is_empty() {
            return Some(idx);
        }
    }
    None
}
