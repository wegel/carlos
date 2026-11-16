//! Adapt complete shared queue input to a private terminal editor.

use anyhow::Result;
use carlos_client::panels::PanelKind;
use carlos_client::Effects;
use ratatui_textarea::TextArea;

use super::queue::QueueEditor;
use super::transport::Transport;
use super::Device;

/// Keep only this terminal's editable draft and its private save counter.
#[derive(Default)]
pub(super) struct Editors {
    pub(super) draft: Option<QueueEditor>,
    pub(super) version: u64,
}

impl Device {
    /// Ask the shared reader for complete text before creating any editable widget.
    pub(super) fn begin_queue_edit(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.require_current_queue()?;
        let effects = self.client.begin_queue_input(&self.session)?;
        self.view.codex_queue.error = None;
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
        }
        Ok(())
    }

    /// Copy only the selected id and transfer complete text when this shared model changes.
    pub(super) fn apply_queue_model(&mut self, effects: &Effects) -> Result<bool> {
        if !effects
            .panels
            .iter()
            .any(|change| change.session == self.session && change.kind == PanelKind::Queue)
        {
            return Ok(false);
        }
        if let Some(panel) = self.queue_panel() {
            self.view.codex_queue.selected = panel.selection().map(str::to_owned);
        }
        if let Some(input) = self.client.take_queue_input(&self.session) {
            self.queue.editing.version =
                self.queue.editing.version.checked_add(1).ok_or_else(|| {
                    anyhow::anyhow!("Queue editor counter exhausted; reopen the terminal")
                })?;
            self.queue.editing.draft = Some(QueueEditor {
                entry: input.entry,
                expected: input.expected,
                version: self.queue.editing.version,
                editor: TextArea::new(input.text.split('\n').map(str::to_string).collect()),
            });
        }
        Ok(true)
    }
}
