// Tests for shell command rendering, SSH rewrites, and ANSI stripping.

#[test]
fn exec_command_end_shell_git_diff_is_summarized_as_diff() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_git_diff\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_git_diff\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"git diff -- src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_git_diff\",\"aggregatedOutput\":\"diff --git a/src/main.rs b/src/main.rs\\n@@ -1 +1 @@\\n-old\\n+new\\n\",\"exitCode\":1},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
}

#[test]
fn exec_command_end_parsed_edit_type_is_summarized_as_edit() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_edit_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_edit_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"edit\",\"path\":\"/repo/src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_edit_1\",\"aggregatedOutput\":\"patched\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].tool_output.as_ref().expect("tool details").summary, "← Edit src/main.rs");
    assert_eq!(app.messages[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_command_execution_diff_output_renders_diff_message_kind() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_diff_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_diff_1\",\"aggregatedOutput\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+new\\n\",\"exitCode\":1},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));
}

#[test]
fn format_tool_item_run_style_from_command_fields() {
    let item = json!({
        "type": "toolCall",
        "input": {
            "command": "cargo test",
            "reasoning": "Running cargo test in repo",
            "description": "Runs Rust test suite using Cargo"
        }
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted tool call");
    assert!(rendered.contains("run cargo test"));
    assert!(rendered.contains("Thinking: Running cargo test in repo"));
    assert!(rendered.contains("# Runs Rust test suite using Cargo"));
    assert!(rendered.contains("$ cargo test"));
}

#[test]
fn format_tool_item_collects_stdout_stderr_and_exit_code() {
    let item = json!({
        "type": "toolOutput",
        "stdout": "Finished `test` profile [optimized + debuginfo] target(s) in 0.04s",
        "stderr": "",
        "exitCode": 0
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert!(rendered.contains("Finished `test` profile"));
    assert!(rendered.contains("exit code: 0"));
}

#[test]
fn format_tool_item_read_call_shows_offset_bracket() {
    let item = json!({
        "type": "toolCall",
        "tool": "read",
        "input": {
            "filePath": "src/main.rs",
            "offset": 1791
        }
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted read call");
    assert_eq!(rendered, "→ Read src/main.rs [offset=1791]");
}

#[test]
fn format_command_execution_call_uses_action_command() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_1",
        "command": "/usr/bin/zsh -lc 'ls -1'",
        "commandActions": [
            { "type": "listFiles", "command": "ls -1", "path": null }
        ],
        "status": "inProgress"
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted command call");
    assert_eq!(rendered, "run ls -1\n$ ls -1");
}

#[test]
fn parse_ssh_remote_command_extracts_destination_and_payload() {
    let parsed = parse_ssh_remote_command(
        "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
    )
    .expect("parsed ssh command");

    assert_eq!(parsed.destination, "wegel@192.168.3.20");
    assert_eq!(
        parsed.remote_command,
        "curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq"
    );
}

#[test]
fn strip_terminal_controls_preserving_sgr_removes_control_noise() {
    let cleaned = strip_terminal_controls_preserving_sgr(
        "\u{1b}[7l\u{1b}[31mhello\u{1b}[0m\n\u{1b}]0;title\u{07}world\u{1b}[?25h",
    );
    assert_eq!(cleaned, "\u{1b}[31mhello\u{1b}[0m\nworld");
}

#[test]
fn strip_terminal_controls_preserving_sgr_handles_unknown_escape_before_unicode() {
    let cleaned = strip_terminal_controls_preserving_sgr("\u{1b}��g��a��������");
    assert_eq!(cleaned, "��g��a��������");
}

#[test]
fn strip_terminal_controls_removes_sgr_sequences_too() {
    let cleaned =
        strip_terminal_controls("\u{1b}[7l\u{1b}[31mhello\u{1b}[0m\n\u{1b}]0;title\u{07}world");
    assert_eq!(cleaned, "hello\nworld");
}

#[test]
fn format_command_execution_call_rewrites_ssh_transport_as_remote_exec() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ssh_1",
        "command": "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
        "status": "inProgress"
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted command call");
    assert_eq!(
        rendered,
        "remote exec on wegel@192.168.3.20\n$ curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq"
    );
}

#[test]
fn format_command_execution_output_uses_aggregated_output() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_1",
        "command": "/usr/bin/zsh -lc 'ls -1'",
        "commandActions": [
            { "type": "listFiles", "command": "ls -1", "path": null }
        ],
        "aggregatedOutput": "a\nb\n",
        "exitCode": 0,
        "durationMs": 51,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted command output");
    assert!(rendered.starts_with("$ ls -1\n"), "rendered={rendered:?}");
    assert!(rendered.contains("a\nb"), "rendered={rendered:?}");
    assert!(rendered.contains("exit code: 0"), "rendered={rendered:?}");
}

#[test]
fn format_command_execution_output_preserves_sgr_sequences_but_strips_control_noise() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ansi_1",
        "command": "/usr/bin/zsh -lc 'printf hello'",
        "aggregatedOutput": "\u{001b}[7l\u{001b}[31mhello\u{001b}[0m\n",
        "exitCode": 0,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert_eq!(
        rendered,
        "$ printf hello\n\u{001b}[31mhello\u{001b}[0m\n\nexit code: 0"
    );
}

#[test]
fn format_command_execution_output_rewrites_ssh_transport_as_remote_exec() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ssh_2",
        "command": "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
        "aggregatedOutput": "{\n  \"state\": \"running\"\n}\n",
        "exitCode": 0,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert_eq!(
        rendered,
        "remote exec on wegel@192.168.3.20\n$ curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq\n{\n  \"state\": \"running\"\n}\n\nexit code: 0"
    );
}
