//! Scroll and copy the client transcript using the terminal's existing gesture and text rules.

use anyhow::Result;
use carlos_proto::session::{ApprovalInput, EntryKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::transcript::Transcript;
use super::Device;
use crate::app::mouse_gestures::{self, MouseHitContext};
use crate::app::render_input::compute_composer_layout;
use crate::app::selection::{normalize_selection_x, selected_text, Selection};
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::TerminalSize;
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};

impl Device {
    pub(super) fn navigation_key(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => self.copy(),
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => self.view.viewport.selection = None,
            (KeyCode::Home, _) if self.view.input.is_empty() => {
                self.view.viewport.scroll_top = 0;
                self.view.viewport.auto_follow_bottom = false;
            }
            (KeyCode::End, _) if self.view.input.is_empty() => {
                self.view.viewport.auto_follow_bottom = true
            }
            (KeyCode::PageUp, _) | (KeyCode::PageDown, _) => {
                let (height, _) = self.geometry()?;
                let max_scroll = self
                    .view
                    .render_cache
                    .rendered_line_count()
                    .saturating_sub(height);
                self.view
                    .viewport
                    .page(key.code == KeyCode::PageDown, max_scroll);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self
            .client
            .session(&self.session)
            .is_some_and(|replica| replica.approvals().iter().any(|card| card.active))
        {
            return Ok(());
        }
        let (height, content_x) = self.geometry()?;
        if height == 0 {
            return Ok(());
        }
        let row = (mouse.row as usize).min(height - 1);
        let hit = MouseHitContext {
            in_messages: (mouse.row as usize) < height,
            norm_x: normalize_selection_x(mouse.column as usize, content_x),
            clamped_y: row + 1,
            clamped_line_idx: self.view.viewport.scroll_top + row,
            max_scroll: self
                .view
                .render_cache
                .rendered_line_count()
                .saturating_sub(height),
            last_line: self
                .view
                .render_cache
                .rendered_line_count()
                .saturating_sub(1),
        };
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                mouse_gestures::wheel(&mut self.view.viewport, &hit, true);
            }
            MouseEventKind::ScrollDown => {
                mouse_gestures::wheel(&mut self.view.viewport, &hit, false);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                mouse_gestures::start(&mut self.view.viewport, &hit);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                mouse_gestures::drag(&mut self.view.viewport, &hit);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let (_, copied) = mouse_gestures::finish(&mut self.view.viewport, &hit);
                if let Some(selection) = copied {
                    self.copy_selection(selection);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn geometry(&self) -> Result<(usize, usize)> {
        let (width, height) = crossterm::terminal::size()?;
        let size = TerminalSize {
            width: width as usize,
            height: height as usize,
        };
        let content_x = if self.view.layout.uses_flow(size) {
            1
        } else {
            2
        };
        Ok((
            compute_composer_layout(&self.view.input, size, content_x).msg_bottom,
            content_x,
        ))
    }

    fn copy(&mut self) {
        if let Some(ApprovalInput::Browser { url }) = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.approvals().iter().find(|card| card.active))
            .and_then(|card| card.input.as_ref())
        {
            let _ = try_copy_clipboard(url);
            return;
        }
        if let Some(selection) = self.view.viewport.selection {
            self.copy_selection(selection);
            return;
        }
        let entry = self.client.session(&self.session).and_then(|replica| {
            replica.entries().rev().find(|entry| {
                entry.kind == EntryKind::Answer
                    && (!entry.text.is_empty()
                        || entry
                            .source
                            .as_ref()
                            .is_some_and(|source| source.bytes.get() > 0))
            })
        });
        let Some(entry) = entry else { return };
        let source = self
            .documents
            .token(&entry.id)
            .and_then(|token| self.client.bodies().source(token));
        let source = source.and_then(|window| Some((window.descriptor()?, window.document()?)));
        if let Some(input) = RenderInput::from_client(entry, source) {
            let backend = try_copy_clipboard(input.text);
            self.view.status_hint = Some(format!(
                "copied last assistant message ({})",
                clipboard_backend_label(backend)
            ));
        } else {
            self.view.status_hint = Some("Answer source is not loaded".into());
        }
    }

    fn copy_selection(&mut self, selection: Selection) {
        let Some(transcript) = Transcript::new(&self.client, &self.session, &self.documents) else {
            return;
        };
        self.view.render_cache.materialize_inputs(
            &transcript,
            selection.anchor_line_idx.min(selection.focus_line_idx),
            selection.anchor_line_idx.max(selection.focus_line_idx),
        );
        let text = selected_text(selection, &self.view.render_cache);
        if !text.is_empty() {
            let _ = try_copy_clipboard(&text);
        }
    }
}
