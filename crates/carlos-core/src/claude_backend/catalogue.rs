//! Stream compact SDK session pages into the daemon's fixed-size picker window.

use std::path::Path;
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use super::event_queue::EventSender;
use super::transport::ControllerTransport;
use crate::launch_cancel::LaunchCancellation;
use crate::saved_sessions::remaining;
use crate::session::models::ThreadSummary;

pub(crate) fn visit_saved_sessions(
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary),
) -> Result<()> {
    remaining(cancel, deadline)?;
    let (events, _receiver) = EventSender::channel(1024);
    let helper = ControllerTransport::start(events)?;
    let emitter = helper.emitter.clone();
    cancel.register(move || emitter.stop())?;
    let mut offset = 0;
    loop {
        let result = helper.call(
            "sessions.list",
            json!({"cwd":cwd,"offset":offset,"limit":64,"compact":true}),
            remaining(cancel, deadline)?,
        )?;
        for row in result["sessions"]
            .as_array()
            .context("SDK session page has no sessions")?
        {
            if let Some(row) = summary(row, cwd) {
                visitor(row);
            }
        }
        let Some(next) = result["nextOffset"].as_u64() else {
            break;
        };
        ensure!(next > offset, "SDK session page did not advance");
        offset = next;
    }
    Ok(())
}

fn summary(row: &Value, cwd: &Path) -> Option<ThreadSummary> {
    Some(ThreadSummary {
        id: row["sessionId"].as_str()?.to_owned(),
        name: row["customTitle"].as_str().map(str::to_owned),
        preview: row["summary"]
            .as_str()
            .unwrap_or("Claude session")
            .to_owned(),
        cwd: row["cwd"].as_str().unwrap_or(cwd.to_str()?).to_owned(),
        created_at: row["createdAt"].as_i64().unwrap_or(0) / 1000,
        updated_at: row["lastModified"].as_i64().unwrap_or(0) / 1000,
    })
}
