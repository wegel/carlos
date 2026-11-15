//! Keep the existing rejection helper while its remaining lookup caller moves into core.

use super::{apply_forked_thread_history, apply_grok_rewound_history};
use crate::app::state::AppState;

impl AppState {
    pub(in crate::app) fn reject_fork_for_test(&mut self, response: &str, codex: bool) -> bool {
        if codex {
            apply_forked_thread_history(self, response).is_err()
        } else {
            apply_grok_rewound_history(self, response, Vec::new()).is_err()
        }
    }
}
