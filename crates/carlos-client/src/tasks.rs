//! Validate task replies against their original typed read targets without retaining panel focus.

use std::collections::HashSet;

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::{SessionId, TaskId, TaskReadId, TaskRevision};
use carlos_proto::tasks::{
    Task, TaskDetail, TaskField, TaskPage, TaskRunSnapshot, TaskText, MAX_PAGE_TASKS,
};
use serde_json::Value;

use crate::client_replies::parse;
use crate::pending::Kind;
use crate::text_ranges::valid_named_range;
use crate::ClientError;

/// Match one bounded metadata page without treating its rows as shared mutable selection.
#[derive(Clone)]
pub(crate) struct TaskPageTarget {
    pub session: SessionId,
    pub expected: Option<TaskRevision>,
    pub after: Option<TaskId>,
    pub limit: usize,
}

/// Pin details and text to the selected metadata revision and opaque task address.
#[derive(Clone)]
pub(crate) struct TaskTarget {
    pub session: SessionId,
    pub expected: TaskRevision,
    pub task: TaskId,
}

/// Read only the named scalar and original UTF-8 byte range of a selected task.
#[derive(Clone)]
pub(crate) struct TaskTextTarget {
    pub target: TaskTarget,
    pub field: TaskField,
    pub offset: ByteOffset,
    pub limit: usize,
}

/// A continued current-run read must not follow another client's replacement read.
#[derive(Clone)]
pub(crate) struct TaskRunTarget {
    pub session: SessionId,
    pub task: TaskId,
    pub read: Option<TaskReadId>,
}

pub(crate) fn read_kind(command: &Command) -> Kind {
    match command {
        Command::TaskList {
            session,
            expected,
            after,
            limit,
            ..
        } => Kind::TaskPage(TaskPageTarget {
            session: session.clone(),
            expected: expected.clone(),
            after: after.clone(),
            limit: usize::from(limit.unwrap_or(32)).clamp(1, 32),
        }),
        Command::TaskDetail {
            session,
            window: _,
            expected,
            task,
        } => Kind::TaskDetail(TaskTarget {
            session: session.clone(),
            expected: expected.clone(),
            task: task.clone(),
        }),
        Command::TaskText {
            session,
            window: _,
            expected,
            task,
            field,
            offset,
            limit,
        } => Kind::TaskText(TaskTextTarget {
            target: TaskTarget {
                session: session.clone(),
                expected: expected.clone(),
                task: task.clone(),
            },
            field: *field,
            offset: *offset,
            limit: limit.unwrap_or(65536).clamp(4, 65536) as usize,
        }),
        Command::TaskRun {
            session,
            task,
            read,
            ..
        } => Kind::TaskRun(TaskRunTarget {
            session: session.clone(),
            task: task.clone(),
            read: read.clone(),
        }),
        _ => unreachable!("typed task read"),
    }
}

pub(crate) fn page(target: TaskPageTarget, value: Value) -> Result<TaskPage, ClientError> {
    let page: TaskPage = parse(value)?;
    require_session(&target.session, &page.session)?;
    if target
        .expected
        .as_ref()
        .is_some_and(|expected| expected != &page.revision)
        || page.tasks.len() > target.limit.min(MAX_PAGE_TASKS)
        || page.has_more && page.tasks.is_empty()
    {
        return Err(invalid(
            "task page does not match its requested revision or row bound",
        ));
    }
    let mut ids = HashSet::new();
    for task in &page.tasks {
        require_task(task)?;
        if target.after.as_ref() == Some(&task.id) || !ids.insert(&task.id) {
            return Err(invalid(
                "task page repeats a task or its continuation cursor",
            ));
        }
    }
    Ok(page)
}

pub(crate) fn detail(target: TaskTarget, value: Value) -> Result<TaskDetail, ClientError> {
    let detail: TaskDetail = parse(value)?;
    require_session(&target.session, &detail.session)?;
    require_task(&detail.task)?;
    if detail.task.id != target.task
        || detail.revision != target.expected
        || detail.properties.len() > 32
    {
        return Err(invalid(
            "task detail does not match its selected task or revision",
        ));
    }
    let mut fields = Vec::new();
    for property in &detail.properties {
        if fields.contains(&property.field)
            || property.text.len() > 2048
            || property.bytes.get() < property.text.len() as u64
            || property.complete != (property.bytes.get() == property.text.len() as u64)
        {
            return Err(invalid(
                "task detail contains an invalid or repeated named preview",
            ));
        }
        fields.push(property.field);
    }
    Ok(detail)
}

pub(crate) fn text(target: TaskTextTarget, value: Value) -> Result<TaskText, ClientError> {
    let text: TaskText = parse(value)?;
    require_session(&target.target.session, &text.session)?;
    if text.task != target.target.task
        || text.revision != target.target.expected
        || text.field != target.field
        || !valid_named_range(
            &text.text,
            text.offset,
            text.next_offset,
            target.offset,
            target.limit.min(65536),
            text.complete,
        )
    {
        return Err(invalid(
            "task text does not match its named field or byte range",
        ));
    }
    Ok(text)
}

pub(crate) fn run(target: TaskRunTarget, value: Value) -> Result<TaskRunSnapshot, ClientError> {
    let run: TaskRunSnapshot = parse(value)?;
    require_session(&target.session, &run.session)?;
    if run.task != target.task
        || target.read.is_some() && run.read != target.read
        || run
            .native_state
            .as_ref()
            .is_some_and(|state| state.len() > 128)
        || run.run.is_some() && !run.known
    {
        return Err(invalid(
            "task run does not match its original selected read",
        ));
    }
    Ok(run)
}

fn require_task(task: &Task) -> Result<(), ClientError> {
    if task.label.len() > 1024 || task.state.len() > 128 {
        return Err(invalid("task metadata exceeds its named preview bounds"));
    }
    Ok(())
}

fn require_session(expected: &SessionId, actual: &SessionId) -> Result<(), ClientError> {
    if expected != actual {
        return Err(ClientError::ForeignSession);
    }
    Ok(())
}

fn invalid(message: &str) -> ClientError {
    ClientError::InvalidPayload(message.into())
}
