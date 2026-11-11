//! Prepare label inputs separately from original commands and their read authority.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::shell_command::analyze_shell_command;
use super::tool_command_view::ToolCommandView;
use super::tool_shell::parse_ssh_remote_command;
use super::tools::{command_summary_from_shell_cmd, tool_command};
use crate::summaries::{command_key, eligibility, presentation_key, CommandLocation, Key};

/// Keep model text, cache identity and read context together for one observed item.
pub(super) struct SummaryInput {
    pub(super) key: Key,
    pub(super) command: String,
    pub(super) location: CommandLocation,
    pub(super) file_based: bool,
}

/// Missing final status fields do not erase a command that was supplied earlier.
pub(super) fn has_command(item: &Value) -> bool {
    raw_command(item).is_some_and(|command| !command.is_empty())
}

/// Normalize before classification and eligibility so a long cd cannot turn a trivial read into an AI job.
pub(super) fn summary_input(item: &Value, workspace: &Path) -> Option<SummaryInput> {
    let raw = raw_command(item)?;
    let (key, command, location) = if let Some(view) = ToolCommandView::from_item(item, workspace) {
        if !view.can_classify() {
            return None;
        }
        let location = CommandLocation {
            cwd: view.cwd,
            read_root: view.read_root,
            directory: view.directory,
        };
        let identity = if item["command"].is_array() {
            item["command"].to_string()
        } else {
            raw.clone()
        };
        (
            presentation_key(&identity, &location),
            view.command,
            location,
        )
    } else {
        let cwd = item["cwd"]
            .as_str()
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    workspace.join(path)
                }
            })
            .unwrap_or_else(|| workspace.to_owned());
        (command_key(&raw), raw, CommandLocation::at(cwd))
    };
    if command_summary_from_shell_cmd(&command, None)
        .is_some_and(|label| !label.starts_with("run "))
    {
        return None;
    }
    let file_based = summary_file_based(&command)?;
    Some(SummaryInput {
        key,
        command,
        location,
        file_based,
    })
}

fn summary_file_based(command: &str) -> Option<bool> {
    let analyzed = analyze_shell_command(command);
    if eligibility::eligible(&analyzed.text) {
        return Some(eligibility::file_based(&analyzed.text));
    }
    let remote = parse_ssh_remote_command(command)?;
    let remote = analyze_shell_command(&remote.remote_command);
    (eligibility::eligible(&remote.text) && !eligibility::file_based(&remote.text)).then_some(false)
}

fn raw_command(item: &Value) -> Option<String> {
    for value in [
        &item["command"],
        &item["input"]["command"],
        &item["args"]["command"],
        &item["arguments"]["command"],
    ] {
        if let Some(text) = value.as_str() {
            return Some(text.to_owned());
        }
    }
    tool_command(item)
}
