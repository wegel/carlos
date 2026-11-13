//! Retain a bounded encoded event suffix and refuse oversized pages before cloning source text.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::Arc;

use carlos_proto::commands::RpcError;
use carlos_proto::events::{Notification, NotificationMessage};
use carlos_proto::ids::Sequence;
use carlos_proto::session::Snapshot;

use crate::session::event_rows::is_tool;
use crate::session::events::{Change, Published};
use crate::session::models::{Message, Role};
use crate::session::state::SessionState;

pub(super) const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_RING_BYTES: usize = 16 * 1024 * 1024;
const MAX_RING_FRAMES: usize = 1024;

/// One encoded notification can be shared by several socket writers without another encode.
#[derive(Clone)]
pub struct EngineFrame {
    pub seq: Sequence,
    pub bytes: Arc<[u8]>,
}

#[derive(Default)]
pub(super) struct EventRing {
    frames: VecDeque<EngineFrame>,
    bytes: usize,
    latest: Sequence,
}

impl EventRing {
    pub(super) fn publish(&mut self, session: &mut SessionState) -> bool {
        let published = session.publish();
        let changed = !published.records().is_empty();
        self.latest = published.sequence();
        for record in published.records() {
            let frame = self.encode(&published, record);
            match frame {
                Ok(frame) => self.push(frame),
                Err(_) => {
                    self.frames.clear();
                    self.bytes = 0;
                }
            }
        }
        changed
    }

    pub(super) fn replay(&self, since: Sequence) -> Option<Vec<EngineFrame>> {
        if since == self.latest {
            return Some(Vec::new());
        }
        if since > self.latest || self.frames.front()?.seq.get() > since.get().checked_add(1)? {
            return None;
        }
        Some(
            self.frames
                .iter()
                .filter(|frame| frame.seq > since)
                .cloned()
                .collect(),
        )
    }

    pub(super) fn sequence(&self) -> Sequence {
        self.latest
    }

    fn push(&mut self, frame: EngineFrame) {
        self.bytes += frame.bytes.len();
        self.frames.push_back(frame);
        while self.bytes > MAX_RING_BYTES || self.frames.len() > MAX_RING_FRAMES {
            self.bytes -= self.frames.pop_front().expect("nonempty ring").bytes.len();
        }
    }

    fn encode(
        &self,
        published: &Published<'_>,
        record: &crate::session::events::NumberedChange,
    ) -> Result<EngineFrame, RpcError> {
        match record.change {
            Change::Reset { .. } => check_snapshot(published.session, 100)?,
            Change::Added(id) | Change::Replaced(id) => {
                let index = published.position(id).ok_or_else(oversized)?;
                check_rows(&published.session.messages()[index..index + 1])?;
            }
            _ => {}
        }
        let event = published.to_wire(record).ok_or_else(oversized)?;
        let mut output = Limited(Vec::new());
        serde_json::to_writer(
            &mut output,
            &NotificationMessage {
                jsonrpc: carlos_proto::commands::RpcVersion::V2,
                notification: Notification::SessionEvent(event),
            },
        )
        .map_err(|_| oversized())?;
        output.write_all(b"\n").map_err(|_| oversized())?;
        Ok(EngineFrame {
            seq: record.seq,
            bytes: output.0.into(),
        })
    }
}

pub(super) fn snapshot(session: &SessionState, limit: usize) -> Result<Snapshot, RpcError> {
    check_snapshot(session, limit)?;
    let snapshot = session.export_snapshot(limit);
    let mut output = Limited(Vec::new());
    serde_json::to_writer(&mut output, &snapshot).map_err(|_| oversized())?;
    Ok(snapshot)
}

fn check_snapshot(session: &SessionState, limit: usize) -> Result<(), RpcError> {
    let start = session
        .messages()
        .len()
        .saturating_sub(limit.clamp(1, 1000));
    let mut bytes = row_bytes(&session.messages()[start..]);
    for card in session.approval().live() {
        let facts = card.facts();
        bytes = bytes.saturating_add(facts.title.len());
        for detail in &facts.detail_lines {
            bytes = bytes.saturating_add(detail.len());
        }
        if let Some(payload) = &facts.requested_permissions {
            let mut size = Count(0);
            serde_json::to_writer(&mut size, payload).map_err(|_| oversized())?;
            bytes = bytes.saturating_add(size.0.saturating_mul(4));
        }
        if bytes > MAX_FRAME {
            return Err(oversized());
        }
    }
    if bytes > MAX_FRAME {
        return Err(oversized());
    }
    Ok(())
}

pub(super) fn check_rows(rows: &[Message]) -> Result<(), RpcError> {
    if row_bytes(rows) > MAX_FRAME {
        Err(oversized())
    } else {
        Ok(())
    }
}

fn row_bytes(rows: &[Message]) -> usize {
    rows.iter().fold(0usize, |sum, row| {
        let text = if !is_tool(row) {
            row.text.len()
        } else {
            row.tool_output
                .as_ref()
                .map(|tool| tool.summary.len())
                .unwrap_or_else(|| {
                    if row.role == Role::ToolCall {
                        row.text.lines().next().map_or(0, str::len)
                    } else {
                        0
                    }
                })
        };
        sum.saturating_add(text).saturating_add(1024)
    })
}

struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FRAME.saturating_sub(self.0.len()) {
            return Err(io::Error::other("wire frame exceeds eight MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Count(usize);
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_FRAME.saturating_sub(self.0) {
            return Err(io::Error::other("wire facts exceed eight MiB"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn oversized() -> RpcError {
    RpcError { code: -32011, message: "requested facts exceed the eight MiB page/frame budget; use a smaller page or bounded output read".into() }
}
