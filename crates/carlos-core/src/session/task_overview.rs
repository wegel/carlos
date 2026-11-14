//! Export only the native activity, quota and accepted-input facts already shown by task panels.

use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskOverviewId, TaskRevision};
use carlos_proto::session::Backend;
use carlos_proto::task_overview::{
    TaskLabel, TaskMainState, TaskOverviewData, TaskOverviewEntry, TaskOverviewField,
    TaskOverviewPage, TaskOverviewText,
};
use carlos_proto::tasks::MAX_PAGE_TASKS;

use super::claude::quota::QuotaWindow;
use super::claude::{ClaudeActivity, NativeState};
use super::codex::task_badge;
use super::state::SessionState;
use super::task_digest::hash_text;
use super::task_rows::clipped;

/// Borrow labels and diagnostics only; accepted prompt source text never enters overview reads.
enum OverviewRow<'a> {
    Main(&'a ClaudeActivity),
    Quota(&'a str, &'a QuotaWindow),
    Prompt {
        id: &'a str,
        consumed: bool,
        priority: &'a str,
    },
    ProcessError(&'a str, &'a str),
}

impl SessionState {
    pub(super) fn task_overview_page(
        &self,
        expected: Option<&TaskRevision>,
        after: Option<&TaskOverviewId>,
        limit: usize,
    ) -> Result<TaskOverviewPage, RpcError> {
        let rows = self.overview_rows();
        let revision = self.overview_revision(&rows);
        require_revision(expected, &revision)?;
        if after.is_some() && expected.is_none() {
            return Err(failure(
                "task overview continuation needs its expected revision",
            ));
        }
        let start = after
            .map(|id| {
                rows.iter()
                    .position(|row| row.id(self) == *id)
                    .map(|index| index + 1)
                    .ok_or_else(|| failure("retired or foreign task overview cursor"))
            })
            .transpose()?
            .unwrap_or(0);
        let end = start
            .saturating_add(limit.clamp(1, MAX_PAGE_TASKS))
            .min(rows.len());
        Ok(TaskOverviewPage {
            session: self.session_id().clone(),
            seq: self.events.seq,
            revision,
            entries: rows[start..end]
                .iter()
                .map(|row| row.export(self))
                .collect(),
            has_more: end < rows.len(),
        })
    }

    pub(super) fn task_overview_text(
        &self,
        expected: &TaskRevision,
        id: &TaskOverviewId,
        field: TaskOverviewField,
        offset: ByteOffset,
        limit: usize,
    ) -> Result<TaskOverviewText, RpcError> {
        let rows = self.overview_rows();
        require_revision(Some(expected), &self.overview_revision(&rows))?;
        let row = rows
            .iter()
            .find(|row| row.id(self) == *id)
            .ok_or_else(|| failure("retired or foreign task overview entry"))?;
        let value = row
            .field(field)
            .ok_or_else(|| failure("this overview entry has no named field"))?;
        let start = usize::try_from(offset.get())
            .ok()
            .filter(|offset| *offset <= value.len())
            .ok_or_else(|| failure("overview text offset exceeds the accepted field"))?;
        let start = floor_utf8(value, start);
        let end = floor_utf8(
            value,
            start.saturating_add(limit.clamp(4, 65536)).min(value.len()),
        );
        Ok(TaskOverviewText {
            session: self.session_id().clone(),
            revision: expected.clone(),
            entry: id.clone(),
            field,
            offset: ByteOffset::new(start as u64),
            next_offset: ByteOffset::new(end as u64),
            complete: end == value.len(),
            text: value[start..end].into(),
        })
    }

    pub(super) fn task_overview_revision(&self) -> TaskRevision {
        self.overview_revision(&self.overview_rows())
    }

    fn overview_rows(&self) -> Vec<OverviewRow<'_>> {
        match self.events.backend {
            Backend::Claude => {
                let mut rows = vec![OverviewRow::Main(self.claude.activity())];
                rows.extend(
                    self.claude
                        .quota()
                        .windows()
                        .iter()
                        .map(|(key, window)| OverviewRow::Quota(key, window)),
                );
                rows.extend(
                    self.claude_prompts()
                        .filter(|(_, prompt)| !prompt.clear())
                        .map(|(id, prompt)| OverviewRow::Prompt {
                            id,
                            consumed: prompt.consumed(),
                            priority: prompt.priority(),
                        }),
                );
                rows
            }
            Backend::Codex => {
                let scope = self.backend_context().panel;
                let mut errors: Vec<_> = self
                    .codex_task_status
                    .errors
                    .iter()
                    .filter(|(owner, _)| task_badge::owns(self, scope, owner))
                    .collect();
                errors.sort_by(|left, right| left.0.cmp(right.0));
                errors
                    .into_iter()
                    .map(|(owner, error)| OverviewRow::ProcessError(owner, error))
                    .collect()
            }
            Backend::Grok => Vec::new(),
        }
    }

    fn overview_revision(&self, rows: &[OverviewRow<'_>]) -> TaskRevision {
        let mut hash = self.task_scope();
        hash.update(b"overview");
        for row in rows {
            hash_text(&mut hash, row.id(self).as_str());
            for field in [TaskOverviewField::Label, TaskOverviewField::Message] {
                hash_text(&mut hash, row.field(field).unwrap_or(""));
            }
            row.hash_state(&mut hash);
        }
        hash.finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("overview revision digest")
    }
}

impl OverviewRow<'_> {
    fn id(&self, session: &SessionState) -> TaskOverviewId {
        let mut hash = session.task_scope();
        let (tag, key) = match self {
            Self::Main(_) => ("main", "main"),
            Self::Quota(key, _) => ("quota", *key),
            Self::Prompt { id, .. } => ("prompt", *id),
            Self::ProcessError(owner, _) => ("process_error", *owner),
        };
        hash_text(&mut hash, tag);
        hash_text(&mut hash, key);
        hash.finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("overview entry address")
    }

    fn field(&self, field: TaskOverviewField) -> Option<&str> {
        match (self, field) {
            (Self::Main(activity), TaskOverviewField::Label) => activity.transient(),
            (Self::Quota(_, window), TaskOverviewField::Label) => Some(window.label()),
            (Self::Prompt { id, .. }, TaskOverviewField::Label) => Some(id),
            (Self::ProcessError(owner, _), TaskOverviewField::Label) => Some(owner),
            (Self::ProcessError(_, error), TaskOverviewField::Message) => Some(error),
            _ => None,
        }
    }

    fn export(&self, session: &SessionState) -> TaskOverviewEntry {
        let data = match self {
            Self::Main(activity) => TaskOverviewData::Main {
                state: main_state(activity),
                thinking_tokens: activity.thinking(),
                transient: activity.transient().map(label),
            },
            Self::Quota(_, window) => TaskOverviewData::Quota {
                label: label(window.label()),
                used_pct: window.used(),
                resets_at: window.reset(),
            },
            Self::Prompt {
                id,
                consumed,
                priority,
            } => TaskOverviewData::Prompt {
                label: label(id),
                consumed: *consumed,
                steer: *priority == "now",
            },
            Self::ProcessError(owner, error) => TaskOverviewData::ProcessError {
                label: label(owner),
                message: label(error),
            },
        };
        TaskOverviewEntry {
            id: self.id(session),
            data,
        }
    }

    fn hash_state(&self, hash: &mut blake3::Hasher) {
        match self {
            Self::Main(activity) => {
                hash.update(&[main_state(activity) as u8]);
                hash_text(hash, &format!("{:?}", activity.thinking()));
            }
            Self::Quota(_, window) => {
                hash.update(&window.used().to_bits().to_le_bytes());
                hash.update(&window.reset().to_le_bytes());
            }
            Self::Prompt {
                consumed, priority, ..
            } => {
                hash.update(&[u8::from(*consumed)]);
                hash_text(hash, priority);
            }
            Self::ProcessError(_, _) => {}
        }
    }
}

fn label(text: &str) -> TaskLabel {
    let preview = clipped(text, 2048);
    TaskLabel {
        text: preview.into(),
        bytes: ByteLength::new(text.len() as u64),
        complete: preview.len() == text.len(),
    }
}

fn main_state(activity: &ClaudeActivity) -> TaskMainState {
    match activity.native_state() {
        NativeState::Idle => TaskMainState::Idle,
        NativeState::Running => TaskMainState::Running,
        NativeState::RequiresAction => TaskMainState::RequiresAction,
    }
}

fn floor_utf8(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn require_revision(
    expected: Option<&TaskRevision>,
    actual: &TaskRevision,
) -> Result<(), RpcError> {
    if expected.is_some_and(|expected| expected != actual) {
        return Err(failure(
            "task overview changed; read a new page before continuing",
        ));
    }
    Ok(())
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
