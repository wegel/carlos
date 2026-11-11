//! Schedule bounded Codex panel reads and exact selected controls without blocking the UI.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_panel_state::{Activity, PanelId, Request, Snapshot, Stop};
use super::state::AppState;
use crate::backend::BackendClient;

/// Poll only while the panel is open; pending controls still settle after it closes.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if !app.codex.enabled || app.codex.closed {
        return false;
    }
    if app.codex.panel.show
        && !app.codex.panel.older_window
        && !app.codex.panel.detail
        && app.codex.panel.snapshot.is_none()
        && app
            .codex
            .panel
            .refreshed
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(3))
    {
        refresh(app, false);
    }
    let mut changed = expire(app);
    while let Some(request) = app.codex.panel.outgoing.pop_front() {
        let context = json!({"operation":"panel","epoch":app.codex.epoch,
            "root":app.thread_id,"token":request.token,"timeoutMs":timeout(app, &request)});
        if let Err(error) =
            client.request_control_with_context(&request.method, request.params.clone(), context)
        {
            super::codex_panel_receive::failed(app, &request, &error.to_string());
        }
        changed = true;
    }
    changed
}

/// Start a recent or explicitly older bounded agent window without replacing a usable snapshot yet.
pub(super) fn refresh(app: &mut AppState, older: bool) {
    if app.codex.closed || app.codex.panel.snapshot.is_some() {
        return;
    }
    let cursor = if older {
        app.codex.panel.older.clone()
    } else {
        Value::Null
    };
    if older && cursor.is_null() {
        return;
    }
    let request = request(
        app,
        "thread/list",
        json!({"ancestorThreadId":app.thread_id,
        "sourceKinds":["subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther"],
        "sortKey":"updated_at","sortDirection":"desc","limit":1,"cursor":cursor}),
    );
    app.codex.panel.snapshot = Some(Snapshot {
        request,
        started: Instant::now(),
        agents: Vec::new(),
        processes: Vec::new(),
        scopes: VecDeque::new(),
        bytes: 0,
        cursors: HashSet::new(),
        revision: app.codex.panel.revision,
        older: Value::Null,
        lineage: HashMap::new(),
        checked: HashSet::new(),
        errors: Vec::new(),
        process_scopes: HashMap::new(),
        older_window: older,
    });
    app.codex.panel.notice = "Reading agents and background processes…".into();
}

/// Inspect a selected child without resuming it or changing its model or history.
pub(super) fn details(app: &mut AppState, older: bool) {
    let Some(PanelId::Agent(thread)) = app.codex.panel.selected.clone() else {
        return;
    };
    if !app.codex.panel.selected_exists() {
        return;
    }
    if !older
        || app
            .codex
            .panel
            .activity
            .as_ref()
            .is_none_or(|activity| activity.thread != thread)
    {
        app.codex.panel.activity = Some(Activity {
            thread: thread.clone(),
            request: None,
            started: Instant::now(),
            items: Vec::new(),
            cursor: Value::Null,
            cursors: HashSet::new(),
            bytes: 0,
            tail: String::new(),
            error: None,
            more: true,
            until: 20,
        });
    }
    let activity = app
        .codex
        .panel
        .activity
        .as_mut()
        .expect("selected activity");
    if activity.request.is_some() || !activity.more || activity.items.len() >= 128 {
        return;
    }
    activity.started = Instant::now();
    activity.until = (activity.items.len() + 20).min(128);
    let params =
        json!({"threadId":thread,"sortDirection":"desc","limit":1,"cursor":activity.cursor});
    let request = request(app, "thread/items/list", params);
    app.codex
        .panel
        .activity
        .as_mut()
        .expect("selected activity")
        .request = Some(request);
    app.codex.panel.detail = true;
    app.codex.panel.scroll = 0;
}

/// Stop only a currently selected verified native identity; never redirect a vanished selection.
pub(super) fn stop(app: &mut AppState) {
    if app.codex.closed || app.codex.panel.stop.is_some() {
        return;
    }
    if !app.codex.panel.selected_exists() {
        app.codex.panel.error =
            Some("Selected row left the list; use arrows to choose another".into());
        return;
    }
    let target = app
        .codex
        .panel
        .selected
        .clone()
        .expect("selected native row");
    let (method, params) = match &target {
        PanelId::Agent(thread) => (
            "thread/turns/list",
            json!({"threadId":thread,
            "sortDirection":"desc","itemsView":"notLoaded","limit":1}),
        ),
        PanelId::Process {
            thread, process, ..
        } => (
            "thread/backgroundTerminals/terminate",
            json!({"threadId":thread,"processId":process}),
        ),
    };
    let request = request(app, method, params);
    app.codex.panel.stop = Some(Stop {
        target,
        request,
        message: "Checking the selected native target…".into(),
    });
    app.codex.panel.control_error = None;
}

/// Queue one bounded operation with a caller-owned token that never reaches native parameters.
pub(super) fn request(app: &mut AppState, method: &str, params: Value) -> Request {
    let tokens = [
        app.codex
            .panel
            .snapshot
            .as_ref()
            .map(|read| read.request.token),
        app.codex
            .panel
            .activity
            .as_ref()
            .and_then(|activity| activity.request.as_ref().map(|request| request.token)),
        app.codex.panel.stop.as_ref().map(|stop| stop.request.token),
    ];
    app.codex
        .panel
        .outgoing
        .retain(|request| tokens.contains(&Some(request.token)));
    app.codex.serial = app.codex.serial.saturating_add(1);
    let request = Request {
        token: app.codex.serial,
        method: method.into(),
        params,
    };
    app.codex.panel.outgoing.push_back(request.clone());
    request
}

/// Continue a page with the same read token and original deadline.
pub(super) fn continue_read(app: &mut AppState, mut request: Request, cursor: Value) -> Request {
    request.params["cursor"] = cursor;
    app.codex.panel.outgoing.push_back(request.clone());
    request
}

fn timeout(app: &AppState, request: &Request) -> u64 {
    let started = app
        .codex
        .panel
        .snapshot
        .as_ref()
        .filter(|read| read.request.token == request.token)
        .map(|read| read.started)
        .or_else(|| {
            app.codex
                .panel
                .activity
                .as_ref()
                .filter(|activity| {
                    activity
                        .request
                        .as_ref()
                        .is_some_and(|pending| pending.token == request.token)
                })
                .map(|activity| activity.started)
        });
    if let Some(started) = started {
        return Duration::from_secs(20)
            .saturating_sub(started.elapsed())
            .as_millis()
            .clamp(1, 5000) as u64;
    }
    if crate::protocol::is_read_only(&request.method) {
        5000
    } else {
        15000
    }
}

fn expire(app: &mut AppState) -> bool {
    let mut expired = Vec::new();
    if let Some(read) = app
        .codex
        .panel
        .snapshot
        .as_ref()
        .filter(|read| read.started.elapsed() >= Duration::from_secs(20))
    {
        expired.push(read.request.token);
        app.codex.panel.snapshot = None;
        app.codex.panel.error =
            Some("Panel refresh exceeded20 seconds; previous rows retained".into());
        app.codex.panel.refreshed = Some(Instant::now());
    }
    if let Some(activity) = app.codex.panel.activity.as_mut().filter(|activity| {
        activity.request.is_some() && activity.started.elapsed() >= Duration::from_secs(20)
    }) {
        expired.push(activity.request.take().expect("expired child read").token);
        activity.error =
            Some("Child read exceeded20 seconds; retained items remain available".into());
    }
    app.codex
        .panel
        .outgoing
        .retain(|request| !expired.contains(&request.token));
    !expired.is_empty()
}

/// Wake an idle UI for periodic F2 reads and original read deadlines without polling hidden views.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    if !app.codex.enabled || app.codex.closed {
        return None;
    }
    let panel = &app.codex.panel;
    let snapshot = panel
        .snapshot
        .as_ref()
        .map(|read| (read.started + Duration::from_secs(20)).saturating_duration_since(now));
    let activity = panel
        .activity
        .as_ref()
        .filter(|activity| activity.request.is_some())
        .map(|activity| {
            (activity.started + Duration::from_secs(20)).saturating_duration_since(now)
        });
    let refresh = (panel.show && !panel.older_window && !panel.detail && panel.snapshot.is_none())
        .then(|| {
            panel.refreshed.map_or(Duration::ZERO, |at| {
                (at + Duration::from_secs(3)).saturating_duration_since(now)
            })
        });
    snapshot.into_iter().chain(activity).chain(refresh).min()
}
