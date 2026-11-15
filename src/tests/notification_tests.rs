//! Keep the remaining native callback checks separate from transcript and context tests.

use serde_json::json;

use super::{handle_notification_line, handle_server_message_line, AppState, ServerRequestAction};

include!("notification_tests/context_and_approvals.rs");
