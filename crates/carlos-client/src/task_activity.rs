//! Validate private child activity replies against their original typed interests and pages.

use std::collections::HashSet;

use carlos_proto::commands::Command;
use carlos_proto::ids::{SessionId, TaskActivityId, TaskActivityItemId, TaskId, TaskRevision};
use carlos_proto::task_activity::{
    TaskActivityHeader, TaskActivityPage, MAX_ACTIVITY_ITEMS, MAX_ACTIVITY_PAGE_BYTES,
    MAX_ACTIVITY_TEXT,
};
use serde_json::Value;

use crate::client_replies::parse;
use crate::pending::Kind;
use crate::{ClientError, ReadReply};

pub(crate) fn reply(kind: Kind, value: Value) -> Result<ReadReply, ClientError> {
    match kind {
        Kind::TaskActivity(target) => header(target, value).map(ReadReply::TaskActivity),
        Kind::TaskActivityPage(target) => page(target, value).map(ReadReply::TaskActivityPage),
        _ => unreachable!("typed child reply"),
    }
}

#[derive(Clone)]
pub(crate) struct ActivityTarget {
    pub session: SessionId,
    pub activity: Option<TaskActivityId>,
    pub task: Option<TaskId>,
}

#[derive(Clone)]
pub(crate) struct PageTarget {
    pub session: SessionId,
    pub activity: TaskActivityId,
    pub revision: TaskRevision,
    pub after: Option<TaskActivityItemId>,
    pub limit: usize,
}

pub(crate) fn read_kind(command: &Command) -> Kind {
    match command {
        Command::TaskActivityOpen { session, task, .. } => Kind::TaskActivity(ActivityTarget {
            session: session.clone(),
            activity: None,
            task: Some(task.clone()),
        }),
        Command::TaskActivityRead {
            session, activity, ..
        }
        | Command::TaskActivityAdvance {
            session, activity, ..
        } => Kind::TaskActivity(ActivityTarget {
            session: session.clone(),
            activity: Some(activity.clone()),
            task: None,
        }),
        Command::TaskActivityPage {
            session,
            activity,
            expected,
            after,
            limit,
        } => Kind::TaskActivityPage(PageTarget {
            session: session.clone(),
            activity: activity.clone(),
            revision: expected.clone(),
            after: after.clone(),
            limit: usize::from(limit.unwrap_or(128)).clamp(1, 128),
        }),
        Command::TaskActivityClose { .. } => Kind::Unit,
        _ => unreachable!("typed child activity read"),
    }
}

pub(crate) fn header(
    target: ActivityTarget,
    value: Value,
) -> Result<TaskActivityHeader, ClientError> {
    let header: TaskActivityHeader = parse(value)?;
    if header.session != target.session {
        return Err(ClientError::ForeignSession);
    }
    if target.activity.is_some_and(|id| id != header.activity)
        || target.task.is_some_and(|id| id != header.task)
        || usize::from(header.items) > MAX_ACTIVITY_ITEMS
        || header.error.as_ref().is_some_and(|text| text.len() > 4096)
        || header.tail.len() > 16 * 1024
    {
        return Err(invalid(
            "child header does not match its original interest or native bounds",
        ));
    }
    Ok(header)
}

pub(crate) fn page(target: PageTarget, value: Value) -> Result<TaskActivityPage, ClientError> {
    let encoded = serde_json::to_vec(&value)
        .map_err(|error| invalid(&error.to_string()))?
        .len();
    let page: TaskActivityPage = parse(value)?;
    if page.session != target.session {
        return Err(ClientError::ForeignSession);
    }
    if page.activity != target.activity
        || page.revision != target.revision
        || page.records.len() > target.limit
        || page.has_more && page.records.is_empty()
        || encoded > MAX_ACTIVITY_PAGE_BYTES + 2048
    {
        return Err(invalid(
            "child page does not match its revision, cursor or byte bound",
        ));
    }
    let mut ids = HashSet::new();
    for record in &page.records {
        if !ids.insert(&record.id)
            || target.after.as_ref() == Some(&record.id)
            || record.turn.len() > 4096
            || record.item.len() > 4096
            || record.kind.len() > 128
            || record.text.len() > MAX_ACTIVITY_TEXT
        {
            return Err(invalid(
                "child page repeats a record or exceeds its display scalar bounds",
            ));
        }
    }
    Ok(page)
}

fn invalid(message: &str) -> ClientError {
    ClientError::InvalidPayload(message.into())
}
