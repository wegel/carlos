//! Export only bounded named child display records and the original interest's state.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{Sequence, TaskActivityItemId, TaskRevision};
use carlos_proto::task_activity::{
    TaskActivityHeader, TaskActivityPage, TaskActivityRecord, MAX_ACTIVITY_ITEMS,
    MAX_ACTIVITY_PAGE_BYTES, MAX_ACTIVITY_TEXT,
};
use serde_json::Value;

use super::state::SessionState;
use super::task_activity::ActivityContext;
use super::task_activity_text::display_item;
use super::task_rows::clipped;
use super::task_windows::{failure, require_revision};

impl SessionState {
    pub(in crate::session) fn task_activity_header(
        &self,
        context: &ActivityContext,
    ) -> Result<TaskActivityHeader, RpcError> {
        let activity = self.activity_for_context(context)?;
        let owner = activity.owner.as_ref().expect("addressed child reader");
        Ok(TaskActivityHeader {
            session: self.session_id().clone(),
            activity: context.activity.clone(),
            task: owner.task.clone(),
            revision: owner.revision(),
            change: Sequence::new(owner.change),
            items: activity.items.len() as u16,
            known: owner.known,
            reading: activity.request.is_some(),
            has_older: activity.more,
            closed: self.codex_scope().closed(),
            error: activity
                .error
                .as_ref()
                .map(|error| clipped(error, 4096).into()),
            tail: activity.tail.clone(),
        })
    }

    pub(in crate::session) fn task_activity_page(
        &self,
        context: &ActivityContext,
        expected: &TaskRevision,
        after: Option<&TaskActivityItemId>,
        limit: usize,
    ) -> Result<TaskActivityPage, RpcError> {
        let activity = self.activity_for_context(context)?;
        let owner = activity.owner.as_ref().expect("addressed child reader");
        require_revision(expected, &owner.revision())?;
        if activity.request.is_some() {
            return Err(failure(
                "native child items are still reading; previous display retained",
            ));
        }
        let entries: Vec<_> = activity.items.iter().rev().collect();
        let start = after
            .map(|after| {
                entries
                    .iter()
                    .position(|entry| item_id(context, entry) == *after)
                    .map(|index| index + 1)
                    .ok_or_else(|| failure("retired or foreign child item cursor"))
            })
            .transpose()?
            .unwrap_or(0);
        let mut records = Vec::new();
        let mut bytes = 0;
        for entry in entries
            .iter()
            .skip(start)
            .take(limit.clamp(1, MAX_ACTIVITY_ITEMS))
        {
            let record = export_record(context, entry);
            let encoded = serde_json::to_vec(&record)
                .expect("typed child record")
                .len();
            if bytes + encoded > MAX_ACTIVITY_PAGE_BYTES {
                break;
            }
            bytes += encoded;
            records.push(record);
        }
        let has_more = start + records.len() < entries.len();
        Ok(TaskActivityPage {
            session: self.session_id().clone(),
            activity: context.activity.clone(),
            revision: expected.clone(),
            records,
            has_more,
        })
    }
}

fn export_record(context: &ActivityContext, entry: &Value) -> TaskActivityRecord {
    let item = &entry["item"];
    let content = display_item(item, MAX_ACTIVITY_TEXT);
    TaskActivityRecord {
        id: item_id(context, entry),
        turn: clipped(entry["turnId"].as_str().unwrap_or("unavailable"), 4096).into(),
        item: clipped(item["id"].as_str().unwrap_or("unavailable"), 4096).into(),
        kind: clipped(item["type"].as_str().unwrap_or("unavailable"), 128).into(),
        text: content.text,
        complete: content.complete,
    }
}

fn item_id(context: &ActivityContext, entry: &Value) -> TaskActivityItemId {
    let mut digest = blake3::Hasher::new();
    digest.update(context.activity.as_str().as_bytes());
    for value in [&entry["turnId"], &entry["item"]["id"]] {
        let value = value.as_str().unwrap_or_default();
        digest.update(&(value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    digest
        .finalize()
        .to_hex()
        .as_str()
        .parse()
        .expect("child item address")
}
