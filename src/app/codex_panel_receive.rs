//! Settle scoped panel reads and selected stops without changing the root turn or transcript.

use std::time::Instant;

use serde_json::{json, Value};

use super::codex_panel_state::{PanelId, Request};
use super::state::AppState;

/// Match native results to the exact root epoch, request token, method and target parameters.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    if params["requestContext"]["operation"] != "panel" {
        return false;
    }
    if params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
        || params["requestContext"]["root"] != app.thread_id
    {
        return true;
    }
    let matches = |request: &Request| {
        params["requestContext"]["token"].as_u64() == Some(request.token)
            && params["requestMethod"] == request.method
            && params["requestParams"] == request.params
    };
    if app
        .codex
        .panel
        .stop
        .as_ref()
        .is_some_and(|stop| matches(&stop.request))
    {
        stop_reply(app, &params["response"]);
    } else if let Some(request) = app
        .codex
        .panel
        .activity
        .as_ref()
        .and_then(|activity| activity.request.as_ref())
        .filter(|request| matches(request))
        .cloned()
    {
        if let Some(error) = params["response"].get("error") {
            failed(app, &request, &error_text(error));
        } else if let Err(error) =
            super::codex_panel_activity::page(app, &params["response"]["result"])
        {
            failed(app, &request, &error);
        }
    } else if let Some(request) = app
        .codex
        .panel
        .snapshot
        .as_ref()
        .map(|read| &read.request)
        .filter(|request| matches(request))
        .cloned()
    {
        if let Some(error) = params["response"].get("error") {
            failed(app, &request, &error_text(error));
        } else if let Err(error) =
            super::codex_panel_pages::page(app, &params["response"]["result"])
        {
            failed(app, &request, &error);
        }
    }
    true
}

/// Keep failures visible while leaving healthy main work and all editable drafts intact.
pub(super) fn failed(app: &mut AppState, request: &Request, message: &str) {
    let message = super::claude_question_state::safe_text(message);
    if app
        .codex
        .panel
        .stop
        .as_ref()
        .is_some_and(|stop| stop.request.token == request.token)
    {
        app.codex.panel.stop = None;
        app.codex.panel.control_error = Some(format!("Selected stop: {message}"));
        app.codex.panel.refreshed = None;
        app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
        if app.codex.panel.show {
            super::codex_panel::refresh(app, false);
        }
    } else if let Some(activity) = app.codex.panel.activity.as_mut().filter(|activity| {
        activity
            .request
            .as_ref()
            .is_some_and(|pending| pending.token == request.token)
    }) {
        activity.request = None;
        activity.error = Some(message);
    } else if app
        .codex
        .panel
        .snapshot
        .as_ref()
        .is_some_and(|read| read.request.token == request.token)
    {
        if request.method == "thread/backgroundTerminals/list" {
            if let Some(read) = app.codex.panel.snapshot.as_mut() {
                let short: String = message.chars().take(256).collect();
                if let Some(owner) = request.params["threadId"].as_str() {
                    read.process_scopes
                        .insert(owner.to_string(), Some(short.clone()));
                }
                read.errors.push(format!(
                    "Processes unavailable for {}: {short}",
                    request.params["threadId"]
                ));
            }
            super::codex_panel_pages::next_scope(app);
            return;
        }
        app.codex.panel.snapshot = None;
        app.codex.panel.error = Some(format!(
            "Panel refresh failed: {message}. Previous rows retained."
        ));
        app.codex.panel.refreshed = Some(Instant::now());
    }
}

fn stop_reply(app: &mut AppState, response: &Value) {
    let stop = app.codex.panel.stop.as_ref().expect("selected stop");
    if let Some(error) = response.get("error") {
        let request = stop.request.clone();
        failed(app, &request, &error_text(error));
        return;
    }
    if stop.request.method == "thread/turns/list" {
        let PanelId::Agent(thread) = stop.target.clone() else {
            return;
        };
        let target = stop.target.clone();
        let turn = response["result"]["data"]
            .as_array()
            .and_then(|data| data.first());
        let turn_id = turn
            .filter(|turn| turn["status"] == "inProgress")
            .and_then(|turn| super::codex_panel_pages::valid_id(&turn["id"]).ok())
            .map(str::to_string);
        if !app.codex.panel.ids().contains(&target) || turn_id.is_none() {
            app.codex.panel.stop = None;
            app.codex.panel.notice =
                "No current native turn to stop; idle does not mean completed".into();
            return;
        }
        let request = super::codex_panel::request(
            app,
            "turn/interrupt",
            json!({"threadId":thread,"turnId":turn_id}),
        );
        let stop = app.codex.panel.stop.as_mut().expect("selected stop");
        stop.request = request;
        stop.message = "Stop sent for selected agent turn; awaiting native result".into();
        return;
    }
    if stop.request.method == "thread/backgroundTerminals/terminate"
        && !response["result"]["terminated"].is_boolean()
    {
        let request = stop.request.clone();
        failed(
            app,
            &request,
            "Native stop reply has no terminated flag; stop not confirmed",
        );
        return;
    }
    app.codex.panel.notice = if stop.request.method == "thread/backgroundTerminals/terminate" {
        if response["result"]["terminated"] == true {
            "Native process terminated; refreshing"
        } else {
            "Native process was not running; refreshing"
        }
    } else {
        "Native interrupt acknowledged; refreshing selected agent state"
    }
    .into();
    let target = stop.target.clone();
    app.codex.panel.stop = None;
    if let PanelId::Process {
        thread,
        process,
        item,
    } = target
    {
        super::codex_task_status::terminated(app, &thread, &process, item.as_deref());
    }
    app.codex.panel.refreshed = None;
    if app.codex.panel.show {
        super::codex_panel::refresh(app, false);
    }
}

fn error_text(error: &Value) -> String {
    super::claude_question_state::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Native panel request failed"),
    )
}
