//! Shorten a proven directory prefix only in screen copies and label requests.

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::models::Role;
use super::shell_directory::{directory_prefix, literal_shell_payload};
use super::state::AppState;
use super::tools::{format_tool_item, tool_command};

/// Keep the displayed program separate from its effective directory and original read authority.
pub(super) struct ToolCommandView {
    pub(super) command: String,
    pub(super) cwd: PathBuf,
    pub(super) read_root: PathBuf,
    pub(super) directory: Option<String>,
}

impl ToolCommandView {
    /// Unusual leading bytes belong to the executable name, not to shell separator whitespace.
    pub(super) fn can_classify(&self) -> bool {
        !self
            .command
            .chars()
            .next()
            .is_some_and(|ch| ch.is_whitespace() || ch.is_control())
    }
    /// Require per-command cwd for relative targets; the session root alone does not prove Bash state.
    pub(super) fn from_item(item: &Value, workspace: &Path) -> Option<Self> {
        let raw = command_payload(item)?;
        let prefix = directory_prefix(&raw)?;
        let path = Path::new(&prefix.directory);
        if path.components().any(|part| part == Component::ParentDir) {
            return None;
        }
        let starting = item["cwd"].as_str().map(PathBuf::from);
        let cwd = if path.is_absolute() {
            path.to_owned()
        } else {
            if prefix.directory != "." && !prefix.directory.starts_with("./") {
                return None;
            }
            starting
                .as_ref()
                .filter(|path| path.is_absolute())?
                .join(path)
        };
        let read_root = starting
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    workspace.join(path)
                }
            })
            .unwrap_or_else(|| workspace.to_owned());
        let directory = directory_hint(&cwd, workspace);
        Some(Self {
            command: prefix.body.to_owned(),
            cwd,
            read_root,
            directory,
        })
    }

    /// Add location text outside the command so equal file names in different directories stay distinct.
    pub(super) fn label(&self, label: String) -> String {
        match &self.directory {
            Some(directory) => match label.split_once('\n') {
                Some((first, rest)) => format!("{first} (in {directory})\n{rest}"),
                None => format!("{label} (in {directory})"),
            },
            None => label,
        }
    }

    fn item<'a>(&self, item: &'a Value) -> Cow<'a, Value> {
        let mut display = item.clone();
        display["command"] = Value::String(self.command.clone());
        display["carlosShellView"] = Value::Bool(true);
        display["cwd"] = Value::String(self.cwd.to_string_lossy().into_owned());
        if let Some(directory) = &self.directory {
            display["carlosDirectory"] = Value::String(directory.clone());
        }
        if item["carlosCommandEcho"] == true {
            if let (Some(command), Some(output)) =
                (item["command"].as_str(), item["output"].as_str())
            {
                let prefix = format!("$ {command}\n");
                if let Some(body) = output.strip_prefix(&prefix) {
                    let location = self
                        .directory
                        .as_ref()
                        .map_or(String::new(), |path| format!("in {path}\n"));
                    display["output"] =
                        Value::String(format!("{location}$ {}\n{body}", self.command));
                }
            }
        }
        Cow::Owned(display)
    }
}

impl AppState {
    /// Format a disposable display copy, leaving native items and approval commands untouched.
    pub(super) fn format_tool_item(&self, item: &Value, role: Role) -> Option<String> {
        let view = ToolCommandView::from_item(item, &self.summaries.cwd);
        let item = view
            .as_ref()
            .map_or(Cow::Borrowed(item), |view| view.item(item));
        format_tool_item(&item, role)
    }

    /// Classify the effective program before considering native labels for an unfiltered command.
    pub(super) fn tool_command_summary(&self, item: &Value) -> Option<String> {
        if let Some(view) = ToolCommandView::from_item(item, &self.summaries.cwd) {
            if !view.can_classify() {
                return None;
            }
            return super::tools::command_summary_from_shell_cmd(&view.command, view.cwd.to_str())
                .map(|label| view.label(label));
        }
        item["id"]
            .as_str()
            .and_then(|id| self.command_override(id))
            .or_else(|| {
                tool_command(item).and_then(|command| {
                    super::tools::command_summary_from_shell_cmd(&command, None)
                })
            })
    }

    /// Infer read coloring from the same display command without opening any source file.
    pub(super) fn tool_source_path(&self, item: &Value) -> Option<String> {
        let view = ToolCommandView::from_item(item, &self.summaries.cwd);
        if view.as_ref().is_some_and(|view| !view.can_classify()) {
            return None;
        }
        let item = view
            .as_ref()
            .map_or(Cow::Borrowed(item), |view| view.item(item));
        super::transcript_source::source_path_from_item(&item)
    }
}

fn directory_hint(cwd: &Path, workspace: &Path) -> Option<String> {
    if cwd == workspace {
        return None;
    }
    let path = cwd.strip_prefix(workspace).unwrap_or(cwd);
    Some(path.to_string_lossy().into_owned())
}

fn command_payload(item: &Value) -> Option<String> {
    for value in [
        &item["command"],
        &item["input"]["command"],
        &item["args"]["command"],
        &item["arguments"]["command"],
    ] {
        if let Some(raw) = value.as_str() {
            return Some(literal_shell_payload(raw).unwrap_or_else(|| raw.to_owned()));
        }
    }
    if let Some(arguments) = item["command"].as_array() {
        if arguments.len() == 3
            && matches!(
                arguments[0].as_str()?.rsplit('/').next()?,
                "sh" | "bash" | "zsh" | "dash"
            )
            && matches!(arguments[1].as_str()?, "-c" | "-lc" | "-cl")
        {
            return arguments[2].as_str().map(str::to_owned);
        }
    }
    // Do not reconstruct shell syntax from argv or an inferred command action.
    None
}
