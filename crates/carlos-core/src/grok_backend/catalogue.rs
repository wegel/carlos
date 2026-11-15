//! Scan local Grok summaries one bounded file at a time for the saved-session picker fallback.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use anyhow::{ensure, Result};

use super::history::summarize_session_contents;
use super::types::grok_sessions_root;
use crate::launch_cancel::LaunchCancellation;
use crate::saved_sessions::remaining;
use crate::session::models::ThreadSummary;

const SUMMARY_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn visit_local_sessions(
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary),
) -> Result<()> {
    let Some(root) = grok_sessions_root() else {
        return Ok(());
    };
    if !root.is_dir() {
        return Ok(());
    }
    for group in fs::read_dir(root)? {
        remaining(cancel, deadline)?;
        let group = group?;
        if !group.file_type()?.is_dir() {
            continue;
        }
        visit_group(&group.path(), cwd, cancel, deadline, visitor)?;
    }
    Ok(())
}

fn visit_group(
    group: &Path,
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary),
) -> Result<()> {
    for entry in fs::read_dir(group)? {
        remaining(cancel, deadline)?;
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(row) = read_summary(&entry.path().join("summary.json"), cwd)? {
            visitor(row);
        }
    }
    Ok(())
}

fn read_summary(path: &Path, cwd: &Path) -> Result<Option<ThreadSummary>> {
    let mut contents = String::new();
    let Ok(file) = File::open(path) else {
        return Ok(None);
    };
    if file
        .take(SUMMARY_BYTES + 1)
        .read_to_string(&mut contents)
        .is_err()
    {
        return Ok(None);
    }
    ensure!(
        contents.len() as u64 <= SUMMARY_BYTES,
        "Grok summary exceeds the eight MiB metadata read budget: {}",
        path.display()
    );
    let Some(row) = summarize_session_contents(path, cwd.to_str().expect("checked cwd"), &contents)
    else {
        return Ok(None);
    };
    Ok(Some(ThreadSummary {
        id: row.session_id,
        name: row.title,
        preview: String::new(),
        cwd: row.cwd,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}
