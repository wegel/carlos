//! HEAD f608424f7ab07404f61e183a30ed2ea155366e39: state_items.rs and state_transcript.rs, copied literally.
// Only imports/type aliases and pub(super) visibility are adapted for the test host.
// Row algorithms and String-to-index maps below must remain unchanged.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use crate::session::models::EntryDraft;

mod items;
pub(crate) mod native;
#[path = "transcript.rs"]
mod state_transcript;
mod models {
    pub(super) use crate::app::{MessageKind, Role};
    pub(super) use crate::session::models::EntryDraft as Message;
}
mod state {
    pub(super) use super::HeadReference as AppState;
}

#[derive(Default)]
pub(crate) struct Context {
    pub(crate) enabled: bool,
    pub(crate) item_turn: Option<String>,
    pub(crate) item_part: usize,
}

#[derive(Default)]
pub(crate) struct HeadReference {
    pub(crate) messages: Vec<EntryDraft>,
    pub(crate) item_to_message_index: HashMap<String, usize>,
    pub(crate) completed_item_ids: HashSet<String>,
    pub(crate) turn_diff_to_index: HashMap<String, usize>,
    pub(crate) command_render_overrides: HashMap<String, String>,
    pub(crate) codex: Context,
}

impl HeadReference {
    // HEAD's cache writes do not change row facts or backend maps.
    fn mark_transcript_dirty_from(&mut self, _: usize) {}

    pub(crate) fn truncate(&mut self, cut: usize) {
        self.messages.truncate(cut);
        self.item_to_message_index.retain(|_, index| *index < cut);
        self.turn_diff_to_index.retain(|_, index| *index < cut);
    }
}
