//! Start, advance and retire explicit child reads through the existing native activity cache.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskActivityId, TaskId, TaskRevision};
use serde_json::{json, Value};

use super::codex::panel;
use super::codex::panel_state::Activity;
use super::state::SessionState;
use super::task_activity::{ActivityContext, ActivityOwner};
use super::task_rows::TaskRef;
use super::task_window_state::{Reader, Scope};
use super::task_windows::{failure, require_revision, WindowContext};

impl SessionState {
    pub(in crate::session) fn open_task_activity(
        &mut self,
        reader: Reader,
        task: TaskId,
        expected: &TaskRevision,
        window: Option<&WindowContext>,
    ) -> Result<ActivityContext, RpcError> {
        self.expire_task_activities();
        if !reader.load(Ordering::Acquire) || self.codex_scope().closed() {
            return Err(failure("the native child reader is unavailable"));
        }
        let rows = self.task_rows_in(window)?;
        require_revision(expected, &self.task_revision_in(window, &rows)?)?;
        let row = rows
            .iter()
            .find(|row| row.id(self) == task)
            .ok_or_else(|| failure("retired or foreign child task"))?;
        if !matches!(row, TaskRef::Agent { .. }) {
            return Err(failure("this task has no native child activity"));
        }
        let thread = row.native_id().to_string();
        if self.codex_panel.activities.len() >= 8 || self.codex_serial() >= u64::MAX - 2 {
            return Err(failure(
                "eight child activity reads are already retained or native tokens exhausted",
            ));
        }
        self.advance_codex_serial();
        let mut digest = self.task_scope();
        digest.update(b"childActivity");
        digest.update(&self.codex_serial().to_le_bytes());
        let activity: TaskActivityId = digest
            .finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("activity address");
        let context = ActivityContext { activity, reader };
        let owner = ActivityOwner {
            context: context.clone(),
            task,
            scope: Scope::capture(self),
            revision: 0,
            change: 0,
            known: false,
            retired: false,
        };
        let key = activity_key(&context.activity);
        self.codex_panel
            .activities
            .insert(key.clone(), new_activity(&thread, Some(owner)));
        self.start_activity_read(&key, false)
            .map_err(|error| failure(&error))?;
        Ok(context)
    }

    pub(in crate::session) fn start_activity_read(
        &mut self,
        key: &str,
        older: bool,
    ) -> Result<(), String> {
        if self.codex_scope().closed() {
            return Err("The native child connection is closed".into());
        }
        let activity = self
            .codex_panel
            .activities
            .get_mut(key)
            .ok_or("No original child activity reader")?;
        if activity.request.is_some() {
            return Err("A native child read is already pending".into());
        }
        if older && (!activity.more || activity.items.len() >= 128) {
            return Ok(());
        }
        if !older {
            activity.items.clear();
            activity.bytes = 0;
            activity.tail.clear();
            activity.cursor = Value::Null;
            activity.cursors.clear();
            activity.more = true;
            if let Some(owner) = &mut activity.owner {
                owner.known = false;
            }
        }
        activity.error = None;
        activity.started = Instant::now();
        activity.until = (activity.items.len() + 20).min(128);
        activity.changed(true);
        let params = json!({"threadId":activity.thread,"sortDirection":"desc","limit":1,"cursor":activity.cursor});
        let request = panel::request(self, "thread/items/list", params);
        self.codex_panel
            .activities
            .get_mut(key)
            .expect("original child activity")
            .request = Some(request);
        Ok(())
    }

    pub(in crate::session) fn activity_for_context(
        &self,
        context: &ActivityContext,
    ) -> Result<&Activity, RpcError> {
        self.codex_panel
            .activities
            .get(&activity_key(&context.activity))
            .filter(|activity| {
                activity
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.available(self, context))
            })
            .ok_or_else(|| {
                failure("retired or foreign child activity; explicitly reopen its original task")
            })
    }

    pub(in crate::session) fn close_task_activities(
        &mut self,
        reader: &Reader,
        id: Option<&TaskActivityId>,
    ) {
        let queued: HashSet<_> = self
            .codex_panel
            .outgoing
            .iter()
            .map(|request| request.token)
            .collect();
        let mut remove = Vec::new();
        self.codex_panel.activities.retain(|_, activity| {
            let Some(owner) = &mut activity.owner else {
                return true;
            };
            if !Arc::ptr_eq(reader, &owner.context.reader)
                || id.is_some_and(|id| id != &owner.context.activity)
            {
                return true;
            }
            owner.retired = true;
            activity.items.clear();
            activity.bytes = 0;
            activity.tail.clear();
            activity.cursors.clear();
            activity.cursor = Value::Null;
            if let Some(request) = &activity.request {
                remove.push(request.token);
                return !queued.contains(&request.token);
            }
            false
        });
        self.codex_panel
            .outgoing
            .retain(|request| !remove.contains(&request.token));
    }

    pub(in crate::session) fn expire_task_activities(&mut self) {
        let scope = Scope::capture(self);
        let dead: Vec<_> = self
            .codex_panel
            .activities
            .values()
            .filter_map(|activity| activity.owner.as_ref())
            .filter(|owner| !owner.context.reader.load(Ordering::Acquire) || owner.scope != scope)
            .map(|owner| owner.context.reader.clone())
            .collect();
        for reader in dead {
            self.close_task_activities(&reader, None);
        }
        self.codex_panel.activities.retain(|_, activity| {
            !activity.owner.as_ref().is_some_and(|owner| owner.retired)
                || activity.request.is_some()
                    && activity.started.elapsed() < Duration::from_secs(20)
        });
    }
}

pub(in crate::session) fn new_activity(thread: &str, owner: Option<ActivityOwner>) -> Activity {
    Activity {
        thread: thread.into(),
        owner,
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
    }
}

impl Activity {
    pub(in crate::session) fn changed(&mut self, items: bool) {
        if let Some(owner) = &mut self.owner {
            owner.changed(items);
        }
    }
}

pub(in crate::session) fn activity_key(id: &TaskActivityId) -> String {
    format!("reader:{id}")
}
