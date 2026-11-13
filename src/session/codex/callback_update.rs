//! Describe the immediate client effects of native callbacks and control replies.

use serde_json::Value;

use super::callback_commands::{CallbackChecked, CallbackRetired};
use super::callback_questions::CallbackEditor;
use super::callbacks::CallbackKey;
use super::goals::GoalUpdate;
use super::panel_scope::PanelUpdate;
use super::queue::QueueUpdate;
use super::settings::SettingsUpdate;
use crate::session::actions::ServerRequestAction;
use crate::session::item_changes::ItemChanges;

/// Return one synchronous native result without retaining a client editor.
pub(crate) struct CallbackUpdate {
    pub(crate) effect: CallbackEffect,
    pub(crate) action: Option<ServerRequestAction>,
}

/// Name the exact local changes needed after the session handles a native frame.
pub(crate) enum CallbackEffect {
    None,
    ReplyFailed {
        id: Value,
        error: String,
    },
    Accepted {
        key: CallbackKey,
        editor: CallbackEditor,
        focus: Option<bool>,
    },
    Retired(CallbackRetired),
    Checked {
        id: Value,
        update: CallbackChecked,
    },
    Items(ItemChanges),
    Goal(GoalUpdate),
    Queue(QueueUpdate),
    Panel(PanelUpdate),
    Controls {
        interrupt: Option<(Vec<CallbackKey>, String)>,
        settings: SettingsUpdate,
        queue: QueueUpdate,
    },
}

impl CallbackUpdate {
    /// Build an immediate result with no follow-up backend action.
    pub(in crate::session) fn new(effect: CallbackEffect) -> Self {
        Self {
            effect,
            action: None,
        }
    }
}
