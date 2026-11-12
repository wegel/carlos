//! Schedule bounded Codex panel reads and exact selected controls without blocking the UI.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{Activity, PanelId, Request, Snapshot, Stop};
use crate::backend::BackendClient;
use crate::session::state::SessionState;

/// Poll only while the panel is open; pending controls still settle after it closes.
pub(crate) fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
) -> bool {
    if !scope.native.enabled || scope.native.closed {
        return false;
    }
    if scope.visible
        && !session.codex_panel.older_window
        && !scope.detail
        && session.codex_panel.snapshot.is_none()
        && session
            .codex_panel
            .refreshed
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(3))
    {
        refresh(session, scope, false);
    }
    let mut changed = expire(session);
    while let Some(request) = session.codex_panel.outgoing.pop_front() {
        let context = json!({"operation":"panel","epoch":scope.native.epoch,
            "root":session.thread_id(),"token":request.token,"timeoutMs":timeout(session, &request)});
        if let Err(error) =
            client.request_control_with_context(&request.method, request.params.clone(), context)
        {
            super::panel_receive::failed(session, scope, update, &request, &error.to_string());
        }
        changed = true;
    }
    changed
}

/// Start a recent or explicitly older bounded agent window without replacing a usable snapshot yet.
pub(crate) fn refresh(session: &mut SessionState, scope: PanelScope<'_>, older: bool) {
    if scope.native.closed || session.codex_panel.snapshot.is_some() {
        return;
    }
    let cursor = if older {
        session.codex_panel.older.clone()
    } else {
        Value::Null
    };
    if older && cursor.is_null() {
        return;
    }
    let request = request(
        session,
        "thread/list",
        json!({"ancestorThreadId":session.thread_id(),
        "sourceKinds":["subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther"],
        "sortKey":"updated_at","sortDirection":"desc","limit":1,"cursor":cursor}),
    );
    session.codex_panel.snapshot = Some(Snapshot {
        request,
        started: Instant::now(),
        agents: Vec::new(),
        processes: Vec::new(),
        scopes: VecDeque::new(),
        bytes: 0,
        cursors: HashSet::new(),
        revision: session.codex_panel.revision,
        older: Value::Null,
        lineage: HashMap::new(),
        checked: HashSet::new(),
        errors: Vec::new(),
        process_scopes: HashMap::new(),
        older_window: older,
    });
    session.codex_panel.notice = "Reading agents and background processes…".into();
}

/// Inspect a selected child without resuming it or changing its model or history.
pub(crate) fn details(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    older: bool,
) {
    let Some(PanelId::Agent(thread)) = scope.selected.cloned() else {
        return;
    };
    if !session.codex_panel.contains_target(scope.selected) {
        return;
    }
    if !older
        || session
            .codex_panel
            .activity
            .as_ref()
            .is_none_or(|activity| activity.thread != thread)
    {
        session.codex_panel.activity = Some(Activity {
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
    let activity = session
        .codex_panel
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
    let request = request(session, "thread/items/list", params);
    session
        .codex_panel
        .activity
        .as_mut()
        .expect("selected activity")
        .request = Some(request);
    update.detail = true;
    update.reset_scroll = true;
}

/// Stop only a currently selected verified native identity; never redirect a vanished selection.
pub(crate) fn stop(session: &mut SessionState, scope: PanelScope<'_>) {
    if scope.native.closed || session.codex_panel.stop.is_some() {
        return;
    }
    if !session.codex_panel.contains_target(scope.selected) {
        session.codex_panel.error =
            Some("Selected row left the list; use arrows to choose another".into());
        return;
    }
    let target = scope.selected.cloned().expect("selected native row");
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
    let request = request(session, method, params);
    session.codex_panel.stop = Some(Stop {
        target,
        request,
        message: "Checking the selected native target…".into(),
    });
    session.codex_panel.control_error = None;
}

/// Queue one bounded operation with a caller-owned token that never reaches native parameters.
pub(crate) fn request(session: &mut SessionState, method: &str, params: Value) -> Request {
    let tokens = [
        session
            .codex_panel
            .snapshot
            .as_ref()
            .map(|read| read.request.token),
        session
            .codex_panel
            .activity
            .as_ref()
            .and_then(|activity| activity.request.as_ref().map(|request| request.token)),
        session
            .codex_panel
            .stop
            .as_ref()
            .map(|stop| stop.request.token),
    ];
    session
        .codex_panel
        .outgoing
        .retain(|request| tokens.contains(&Some(request.token)));
    session.advance_codex_serial();
    let request = Request {
        token: session.codex_serial(),
        method: method.into(),
        params,
    };
    session.codex_panel.outgoing.push_back(request.clone());
    request
}

/// Continue a page with the same read token and original deadline.
pub(crate) fn continue_read(
    session: &mut SessionState,
    mut request: Request,
    cursor: Value,
) -> Request {
    request.params["cursor"] = cursor;
    session.codex_panel.outgoing.push_back(request.clone());
    request
}

fn timeout(session: &SessionState, request: &Request) -> u64 {
    let started = session
        .codex_panel
        .snapshot
        .as_ref()
        .filter(|read| read.request.token == request.token)
        .map(|read| read.started)
        .or_else(|| {
            session
                .codex_panel
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

fn expire(session: &mut SessionState) -> bool {
    let mut expired = Vec::new();
    if let Some(read) = session
        .codex_panel
        .snapshot
        .as_ref()
        .filter(|read| read.started.elapsed() >= Duration::from_secs(20))
    {
        expired.push(read.request.token);
        session.codex_panel.snapshot = None;
        session.codex_panel.error =
            Some("Panel refresh exceeded20 seconds; previous rows retained".into());
        session.codex_panel.refreshed = Some(Instant::now());
    }
    if let Some(activity) = session.codex_panel.activity.as_mut().filter(|activity| {
        activity.request.is_some() && activity.started.elapsed() >= Duration::from_secs(20)
    }) {
        expired.push(activity.request.take().expect("expired child read").token);
        activity.error =
            Some("Child read exceeded20 seconds; retained items remain available".into());
    }
    session
        .codex_panel
        .outgoing
        .retain(|request| !expired.contains(&request.token));
    !expired.is_empty()
}

/// Wake an idle UI for periodic F2 reads and original read deadlines without polling hidden views.
pub(crate) fn wait(
    session: &SessionState,
    scope: PanelScope<'_>,
    now: Instant,
) -> Option<Duration> {
    if !scope.native.enabled || scope.native.closed {
        return None;
    }
    let panel = &session.codex_panel;
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
    let refresh =
        (scope.visible && !panel.older_window && !scope.detail && panel.snapshot.is_none()).then(
            || {
                panel.refreshed.map_or(Duration::ZERO, |at| {
                    (at + Duration::from_secs(3)).saturating_duration_since(now)
                })
            },
        );
    snapshot.into_iter().chain(activity).chain(refresh).min()
}
