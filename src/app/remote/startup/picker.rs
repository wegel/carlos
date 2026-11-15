//! Keep page navigation, archive consent and selected opaque choices on this terminal.

use anyhow::Result;
use carlos_proto::ids::{SavedChoiceId, SavedScopeId};
use carlos_proto::startup::{OpenTarget, SavedDirection, SavedPage};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Size;

use super::{Bootstrap, Phase};
use crate::app::notifications::is_ctrl_char;
use crate::app::picker_render::compute_picker_layout;
use crate::app::remote::transport::Transport;
use crate::app::terminal_ui::move_picker_selection;
use crate::app::TerminalSize;

#[derive(Clone, Copy)]
pub(super) enum Landing {
    First,
    Last,
    Index(usize),
    FromEnd(usize),
}

pub(super) struct Picker {
    pub page: SavedPage,
    pub selected: usize,
    pub top: usize,
    pub confirm: bool,
    pub stale: bool,
    pub landing: Landing,
}

enum Action {
    None,
    Exit,
    Open {
        scope: SavedScopeId,
        choice: SavedChoiceId,
    },
    Archive {
        scope: SavedScopeId,
        choice: SavedChoiceId,
    },
    Page(SavedDirection, Landing),
}

impl Landing {
    pub(super) fn index(self, len: usize) -> usize {
        match self {
            Self::First => 0,
            Self::Last => len.saturating_sub(1),
            Self::Index(index) => index.min(len.saturating_sub(1)),
            Self::FromEnd(offset) => len.saturating_sub(offset + 1),
        }
    }
}

impl Picker {
    fn key(&mut self, key: KeyEvent) -> Action {
        if is_ctrl_char(key.code, key.modifiers, 'c') {
            return Action::Exit;
        }
        if self.confirm {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.confirm = false;
                    Action::None
                }
                KeyCode::Enter | KeyCode::Char('y') => self.target(true),
                _ => Action::None,
            };
        }
        if key.code == KeyCode::Esc {
            return Action::Exit;
        }
        if let Some(action) = self.page_key(key.code) {
            return action;
        }
        if let Some(index) = move_picker_selection(key.code, self.selected, self.page.rows.len()) {
            self.selected = index;
        } else if key.code == KeyCode::Enter {
            return self.target(false);
        } else if key.code == KeyCode::Char('d') && self.page.can_archive {
            self.confirm = !self.page.rows.is_empty();
        }
        Action::None
    }

    fn page_key(&self, code: KeyCode) -> Option<Action> {
        let len = self.page.rows.len();
        Some(match code {
            KeyCode::Home | KeyCode::Char('g') => {
                Action::Page(SavedDirection::Newest, Landing::First)
            }
            KeyCode::End | KeyCode::Char('G') => {
                Action::Page(SavedDirection::Oldest, Landing::Last)
            }
            KeyCode::Char('l') if self.page.has_older => {
                Action::Page(SavedDirection::Older, Landing::First)
            }
            KeyCode::Char('h') if self.page.has_newer => {
                Action::Page(SavedDirection::Newer, Landing::Last)
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.selected + 1 == len && self.page.has_older =>
            {
                Action::Page(SavedDirection::Older, Landing::First)
            }
            KeyCode::Up | KeyCode::Char('k') if self.selected == 0 && self.page.has_newer => {
                Action::Page(SavedDirection::Newer, Landing::Last)
            }
            KeyCode::PageDown if self.selected + 10 >= len && self.page.has_older => Action::Page(
                SavedDirection::Older,
                Landing::Index(self.selected + 10 - len),
            ),
            KeyCode::PageUp if self.selected < 10 && self.page.has_newer => {
                Action::Page(SavedDirection::Newer, Landing::FromEnd(9 - self.selected))
            }
            _ => return None,
        })
    }

    fn target(&self, archive: bool) -> Action {
        let Some(row) = self.page.rows.get(self.selected).filter(|_| !self.stale) else {
            return Action::None;
        };
        let scope = self.page.scope.clone();
        let choice = row.choice.clone();
        if archive {
            Action::Archive { scope, choice }
        } else {
            Action::Open { scope, choice }
        }
    }
}

impl Bootstrap {
    pub(super) fn key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        if is_ctrl_char(key.code, key.modifiers, 'c') {
            return Ok(true);
        }
        if key.code == KeyCode::Esc && self.picker.as_ref().is_none_or(|picker| !picker.confirm) {
            return Ok(true);
        }
        if self.read.is_some() || matches!(self.phase, Phase::Opening | Phase::Archiving) {
            return Ok(false);
        }
        if key.code == KeyCode::Char('r') && self.picker.is_some() {
            if let Some(link) = link {
                self.list(link)?;
            }
            return Ok(false);
        }
        let action = self
            .picker
            .as_mut()
            .map(|picker| picker.key(key))
            .unwrap_or(Action::None);
        self.picker_action(link, action)
    }

    fn picker_action(&mut self, link: Option<&mut Transport>, action: Action) -> Result<bool> {
        if matches!(action, Action::Exit) {
            return Ok(true);
        }
        let Some(link) = link.filter(|link| self.client.is_ready(link.connection)) else {
            return Ok(false);
        };
        match action {
            Action::Open { scope, choice } => {
                self.open(link, OpenTarget::Choice { scope, choice })?
            }
            Action::Archive { scope, choice } => self.archive(link, scope, choice)?,
            Action::Page(direction, landing) => self.page(link, direction, landing)?,
            Action::None | Action::Exit => {}
        }
        Ok(false)
    }

    pub(super) fn mouse(
        &mut self,
        link: Option<&mut Transport>,
        size: Size,
        event: MouseEvent,
    ) -> Result<()> {
        if self.read.is_some() || !matches!(self.phase, Phase::Picking) {
            return Ok(());
        }
        let Some(picker) = &mut self.picker else {
            return Ok(());
        };
        if picker.confirm && matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            picker.confirm = false;
            return Ok(());
        }
        let action = match event.kind {
            MouseEventKind::ScrollUp => picker.key(KeyEvent::from(KeyCode::Up)),
            MouseEventKind::ScrollDown => picker.key(KeyEvent::from(KeyCode::Down)),
            MouseEventKind::Down(MouseButton::Left) if !picker.confirm => {
                let layout = compute_picker_layout(TerminalSize {
                    width: size.width as usize,
                    height: size.height as usize,
                });
                let row = event.row as usize;
                if row < layout.list_y + 1 || row >= layout.list_y + layout.list_h {
                    return Ok(());
                }
                let index = picker.top + row - layout.list_y - 1;
                if index >= picker.page.rows.len() {
                    return Ok(());
                }
                picker.selected = index;
                picker.target(false)
            }
            _ => Action::None,
        };
        self.picker_action(link, action)?;
        Ok(())
    }
}
