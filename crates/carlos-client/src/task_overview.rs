//! Match bounded readonly task overview replies to their original page or named scalar request.

use std::collections::HashSet;

use carlos_proto::bytes::ByteOffset;
use carlos_proto::ids::{SessionId, TaskOverviewId, TaskRevision};
use carlos_proto::task_overview::{
    TaskLabel, TaskOverviewData, TaskOverviewField, TaskOverviewPage, TaskOverviewText,
};
use carlos_proto::tasks::MAX_PAGE_TASKS;
use serde_json::Value;

use crate::client_replies::parse;
use crate::text_ranges::valid_named_range;
use crate::ClientError;

#[derive(Clone)]
pub(crate) struct OverviewTarget {
    pub session: SessionId,
    pub expected: Option<TaskRevision>,
    pub after: Option<TaskOverviewId>,
    pub limit: usize,
}

#[derive(Clone)]
pub(crate) struct OverviewTextTarget {
    pub session: SessionId,
    pub expected: TaskRevision,
    pub entry: TaskOverviewId,
    pub field: TaskOverviewField,
    pub offset: ByteOffset,
    pub limit: usize,
}

pub(crate) fn page(target: OverviewTarget, value: Value) -> Result<TaskOverviewPage, ClientError> {
    let page: TaskOverviewPage = parse(value)?;
    if page.session != target.session {
        return Err(ClientError::ForeignSession);
    }
    if target
        .expected
        .as_ref()
        .is_some_and(|expected| expected != &page.revision)
        || page.entries.len() > target.limit.min(MAX_PAGE_TASKS)
        || page.has_more && page.entries.is_empty()
    {
        return Err(invalid(
            "task overview differs from its original revision or row bound",
        ));
    }
    let mut ids = HashSet::new();
    for entry in &page.entries {
        if target.after.as_ref() == Some(&entry.id) || !ids.insert(&entry.id) {
            return Err(invalid(
                "task overview repeats a row or its continuation cursor",
            ));
        }
        check_data(&entry.data)?;
    }
    Ok(page)
}

pub(crate) fn text(
    target: OverviewTextTarget,
    value: Value,
) -> Result<TaskOverviewText, ClientError> {
    let text: TaskOverviewText = parse(value)?;
    if text.session != target.session {
        return Err(ClientError::ForeignSession);
    }
    if text.entry != target.entry
        || text.revision != target.expected
        || text.field != target.field
        || !valid_named_range(
            &text.text,
            text.offset,
            text.next_offset,
            target.offset,
            target.limit,
            text.complete,
        )
    {
        return Err(invalid(
            "task overview text differs from its original named field or interval",
        ));
    }
    Ok(text)
}

fn check_data(data: &TaskOverviewData) -> Result<(), ClientError> {
    match data {
        TaskOverviewData::Main { transient, .. } => {
            if let Some(label) = transient {
                check_label(label)?;
            }
        }
        TaskOverviewData::Quota {
            label, used_pct, ..
        } => {
            check_label(label)?;
            if !used_pct.is_finite() || !(0.0..=100.0).contains(used_pct) {
                return Err(invalid("task quota is not a finite used percentage"));
            }
        }
        TaskOverviewData::Prompt { label, .. } => check_label(label)?,
        TaskOverviewData::ProcessError { label, message } => {
            check_label(label)?;
            check_label(message)?;
        }
    }
    Ok(())
}

fn check_label(label: &TaskLabel) -> Result<(), ClientError> {
    if label.text.len() > 2048
        || label.bytes.get() < label.text.len() as u64
        || label.complete != (label.bytes.get() == label.text.len() as u64)
    {
        return Err(invalid(
            "task overview label does not match its declared byte length",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> ClientError {
    ClientError::InvalidPayload(message.into())
}
