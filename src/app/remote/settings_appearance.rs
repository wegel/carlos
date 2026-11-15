//! Save one accepted set of device appearance choices without holding the terminal input loop.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::app::layout_state::ThemePreferences;
use crate::app::tool_output_preferences::save_appearance;
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::view_state::ViewState;

#[derive(Clone, Copy)]
pub(super) struct Appearance {
    pub mode: ToolOutputMode,
    pub themes: ThemePreferences,
}

pub(super) struct Save {
    reply: Receiver<Result<(), String>>,
}

impl Appearance {
    pub fn capture(view: &ViewState) -> Self {
        Self {
            mode: view.runtime_settings.model_settings_tool_output,
            themes: view.layout.draft,
        }
    }

    pub fn save(self) -> Save {
        let (sender, reply) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let result = save_appearance(self.mode, self.themes).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        Save { reply }
    }
}

impl Save {
    pub fn poll(&self) -> Option<Result<(), String>> {
        match self.reply.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(
                "appearance writer stopped before returning its result".into(),
            )),
        }
    }
}
