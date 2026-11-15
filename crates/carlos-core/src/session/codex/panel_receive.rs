//! Settle scoped panel reads and selected stops without changing the root turn or transcript.

use std::time::Instant;

use serde_json::{json, Value};

use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{PanelId, Request};
use crate::session::state::SessionState;

/// Match native results to the exact root epoch, request token, method and target parameters.
pub fn reply(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    params: &Value,
) -> bool {
    if params["requestContext"]["operation"] != "panel" {
        return false;
    }
    if params["requestContext"]["epoch"].as_u64() != Some(scope.native.epoch)
        || params["requestContext"]["root"] != *session.thread_id()
    {
        return true;
    }
    let matches = |request: &Request| {
        params["requestContext"]["token"].as_u64() == Some(request.token)
            && params["requestMethod"] == request.method
            && params["requestParams"] == request.params
    };
    if session.receive_task_window(params) {
        return true;
    }
    if session
        .codex_panel
        .stop
        .as_ref()
        .is_some_and(|stop| matches(&stop.request))
    {
        stop_reply(session, scope, update, &params["response"]);
        return true;
    }
    let token = params["requestContext"]["token"].as_u64();
    let request = session
        .codex_panel
        .run_request(token)
        .or_else(|| session.codex_panel.child_request(token))
        .or_else(|| {
            session
                .codex_panel
                .snapshot
                .as_ref()
                .map(|read| &read.request)
        })
        .filter(|request| matches(request))
        .cloned();
    if let Some(request) = request {
        if let Some(error) = params["response"].get("error") {
            failed(session, scope, update, &request, &error_text(error));
        } else if let Err(error) = apply_page(
            session,
            scope,
            update,
            &request,
            &params["response"]["result"],
        ) {
            failed(session, scope, update, &request, &error);
        }
    }
    true
}

fn apply_page(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    request: &Request,
    result: &Value,
) -> Result<(), String> {
    let process = session
        .codex_panel
        .runs
        .iter()
        .find(|(_, read)| {
            read.process.is_some()
                && read
                    .request
                    .as_ref()
                    .is_some_and(|pending| pending.token == request.token)
        })
        .map(|(key, _)| key.clone());
    if let Some(key) = process {
        return super::panel_process_runs::page(session, &key, result);
    }
    match request.method.as_str() {
        "thread/turns/list" => super::panel_runs::reply(
            session,
            request.params["threadId"]
                .as_str()
                .expect("native run target"),
            result,
        ),
        "thread/items/list" => super::panel_activity::page(
            session,
            request.params["threadId"].as_str().expect("child target"),
            result,
        ),
        _ => super::panel_pages::page(session, scope, update, result),
    }
}

/// Keep failures visible while leaving healthy main work and all editable drafts intact.
pub fn failed(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    request: &Request,
    message: &str,
) {
    if session.fail_task_window(request, message) {
        return;
    }
    let message = crate::session::text::safe_text(message);
    if session
        .codex_panel
        .stop
        .as_ref()
        .is_some_and(|stop| stop.request.token == request.token)
    {
        session.codex_panel.stop = None;
        session.codex_panel.control_error = Some(format!("Selected stop: {message}"));
        session.codex_panel.refreshed = None;
        session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
        if scope.visible {
            super::panel::refresh(session, scope, false);
        }
    } else if let Some(key) = session
        .codex_panel
        .runs
        .iter()
        .find(|(_, read)| {
            read.request
                .as_ref()
                .is_some_and(|pending| pending.token == request.token)
        })
        .map(|(key, _)| key.clone())
    {
        super::panel_runs::failed(session, &key, &message);
    } else if let Some(activity) = session.codex_panel.child_read_mut(request.token) {
        activity.request = None;
        activity.error = Some(message);
    } else if session
        .codex_panel
        .snapshot
        .as_ref()
        .is_some_and(|read| read.request.token == request.token)
    {
        if request.method == "thread/backgroundTerminals/list" {
            super::panel_pages::process_failed(session, scope, update, &message);
            return;
        }
        session.codex_panel.snapshot = None;
        session.codex_panel.error = Some(format!(
            "Panel refresh failed: {message}. Previous rows retained."
        ));
        session.codex_panel.refreshed = Some(Instant::now());
    }
}

fn stop_reply(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    response: &Value,
) {
    let stop = session.codex_panel.stop.as_ref().expect("selected stop");
    if let Some(error) = response.get("error") {
        let request = stop.request.clone();
        failed(session, scope, update, &request, &error_text(error));
        return;
    }
    if stop.request.method == "thread/turns/list" {
        advance_agent_stop(session, response);
        return;
    }
    if stop.request.method == "thread/backgroundTerminals/terminate"
        && !response["result"]["terminated"].is_boolean()
    {
        let request = stop.request.clone();
        failed(
            session,
            scope,
            update,
            &request,
            "Native stop reply has no terminated flag; stop not confirmed",
        );
        return;
    }
    session.codex_panel.notice = if stop.request.method == "thread/backgroundTerminals/terminate" {
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
    session.codex_panel.stop = None;
    if let PanelId::Process {
        thread,
        process,
        item,
    } = target
    {
        super::task_status::terminated(session, &thread, &process, item.as_deref());
    }
    session.codex_panel.refreshed = None;
    if scope.visible {
        super::panel::refresh(session, scope, false);
    }
}

fn error_text(error: &Value) -> String {
    crate::session::text::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Native panel request failed"),
    )
}

fn advance_agent_stop(session: &mut SessionState, response: &Value) {
    let stop = session.codex_panel.stop.as_ref().expect("selected stop");
    let PanelId::Agent(thread) = stop.target.clone() else {
        return;
    };
    let target = stop.target.clone();
    let turn = response["result"]["data"]
        .as_array()
        .and_then(|data| data.first());
    let turn_id = turn
        .filter(|turn| turn["status"] == "inProgress")
        .and_then(|turn| super::panel_pages::valid_id(&turn["id"]).ok())
        .map(str::to_string);
    if !session.codex_panel.ids().contains(&target) || turn_id.is_none() {
        session.codex_panel.stop = None;
        session.codex_panel.notice =
            "No current native turn to stop; idle does not mean completed".into();
        return;
    }
    let request = super::panel::request(
        session,
        "turn/interrupt",
        json!({"threadId":thread,"turnId":turn_id}),
    );
    let stop = session.codex_panel.stop.as_mut().expect("selected stop");
    stop.request = request;
    stop.message = "Stop sent for selected agent turn; awaiting native result".into();
}
