//! Route native callbacks and owned controls before filtering child-thread output.

use serde_json::Value;

use super::{checked, receive, resolved};
use crate::app::notifications::ServerRequestAction;
use crate::app::state::AppState;

/// Tell the shared parser whether a native callback event was consumed.
pub(in crate::app) enum CallbackRoute {
    Continue,
    Handled(Option<ServerRequestAction>),
}

/// Route requests and resolutions before the main-thread transcript filter.
pub(in crate::app) fn route(app: &mut AppState, method: &str, frame: &Value) -> CallbackRoute {
    if frame.get("id").is_some() {
        return CallbackRoute::Handled(receive(app, method, frame));
    }
    let action = match method {
        "serverRequest/resolved" => {
            resolved(app, &frame["params"]);
            None
        }
        "carlos/codex/formChecked" => checked(app, &frame["params"]),
        "carlos/codex/controlReply" => {
            if crate::app::codex_task_status::reply(app, &frame["params"]) {
                return CallbackRoute::Handled(None);
            }
            if crate::app::codex_reconnect_runtime::reply(app, &frame["params"]) {
                return CallbackRoute::Handled(None);
            }
            if crate::app::codex_reconnect_streams::reply(app, &frame["params"]) {
                return CallbackRoute::Handled(None);
            }
            if crate::app::codex_goal_receive::reply(app, &frame["params"]) {
                return CallbackRoute::Handled(None);
            }
            if crate::app::codex_panel_receive::reply(app, &frame["params"]) {
                return CallbackRoute::Handled(None);
            }
            crate::app::codex_reply::control_reply(app, &frame["params"]);
            crate::app::codex_settings::reply(app, &frame["params"]);
            return CallbackRoute::Handled(crate::app::codex_queue_receive::reply(
                app,
                &frame["params"],
            ));
        }
        "thread/goal/updated" | "thread/goal/cleared" => {
            crate::app::codex_goal_receive::observed(app, method, &frame["params"]);
            None
        }
        "thread/queue/changed" => {
            crate::app::codex_queue_receive::changed(app, &frame["params"]);
            None
        }
        "thread/settings/updated" => {
            crate::app::codex_settings::observed(app, &frame["params"]);
            None
        }
        _ => return CallbackRoute::Continue,
    };
    CallbackRoute::Handled(action)
}
