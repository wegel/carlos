//! Draw command previews and structurally known directory hints without reinterpreting normalized bodies.

use serde_json::Value;

use crate::app::shell_command::analyze_shell_command;
use crate::app::shell_words::{next_word, program_name};
use crate::app::tool_shell::{parse_ssh_remote_command, ParsedSshCommand};
use crate::app::tools::{tool_command, tool_description, tool_output_text, tool_reasoning};

/// Format one call with a compact first-line label and an intact command body.
pub(super) fn format_command_call(item: &Value, cmd: &str) -> String {
    let mut lines = Vec::new();
    if let Some(ssh) = ssh_command(item, cmd) {
        lines.push(format!(
            "remote exec on {}{}",
            ssh.destination,
            directory_suffix(item)
        ));
        push_reasoning_and_description(&mut lines, item);
        lines.push(format!("$ {}", ssh.remote_command));
    } else {
        let first = if item["carlosShellView"] == true {
            cmd.split('\n').next().unwrap_or(cmd)
        } else {
            cmd
        };
        lines.push(format!("run {first}{}", directory_suffix(item)));
        push_reasoning_and_description(&mut lines, item);
        lines.push(format!("$ {cmd}"));
    }
    lines.join("\n")
}

fn push_reasoning_and_description(lines: &mut Vec<String>, item: &Value) {
    if let Some(reason) = tool_reasoning(item) {
        lines.push(format!("Thinking: {reason}"));
    }
    if let Some(desc) = tool_description(item) {
        lines.push(format!("# {desc}"));
    }
}

/// Keep command output separate from its visible directory metadata.
pub(super) fn format_command_output(item: &Value) -> Option<String> {
    let cmd = tool_command(item);
    let output = tool_output_text(item);
    let text = match (cmd, output) {
        (Some(cmd), Some(out)) => Some(format!("{}\n{out}", format_cmd_prefix(item, &cmd))),
        (Some(cmd), None) => Some(format_cmd_prefix(item, &cmd)),
        (None, Some(out)) => Some(out),
        (None, None) => None,
    }?;
    Some(match item["carlosDirectory"].as_str() {
        Some(directory) => format!("in {directory}\n{text}"),
        None => text,
    })
}

fn directory_suffix(item: &Value) -> String {
    item["carlosDirectory"]
        .as_str()
        .map_or(String::new(), |path| format!(" (in {path})"))
}

fn format_cmd_prefix(item: &Value, cmd: &str) -> String {
    if let Some(ssh) = ssh_command(item, cmd) {
        format!(
            "remote exec on {}\n$ {}",
            ssh.destination, ssh.remote_command
        )
    } else {
        format!("$ {cmd}")
    }
}

fn ssh_command(item: &Value, cmd: &str) -> Option<ParsedSshCommand> {
    if item["carlosShellView"] == true
        && (cmd
            .chars()
            .next()
            .is_some_and(|ch| ch.is_whitespace() || ch.is_control())
            || next_word(cmd).is_none_or(|(program, _)| program_name(&program) != "ssh"))
    {
        return None;
    }
    parse_ssh_remote_command(cmd).filter(|_| !analyze_shell_command(cmd).has_rtk)
}
