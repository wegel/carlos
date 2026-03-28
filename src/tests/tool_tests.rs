use super::*;

#[test]
fn exec_command_end_read_override_suppresses_large_read_output_on_success() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_read_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"read\",\"cmd\":\"cat src/main.rs\",\"name\":\"main.rs\",\"path\":\"/repo/src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_1\",\"aggregatedOutput\":\"line1\\nline2\\nline3\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "→ Read src/main.rs");
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_search_override_suppresses_large_output_on_success() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"search\",\"cmd\":\"rg -n foo src/main.rs\",\"path\":\"/repo/src/main.rs\",\"pattern\":\"foo\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_1\",\"aggregatedOutput\":\"1:fn a()\\n2:fn b()\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "✱ Search src/main.rs [pattern=foo]");
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_search_override_kept_on_error_with_output() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_err\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_err\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"search\",\"cmd\":\"rg -n foo src/main.rs\",\"path\":\"/repo/src/main.rs\",\"pattern\":\"foo\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_err\",\"command\":\"/usr/bin/zsh -lc 'rg -n foo src/main.rs missing.rs'\",\"aggregatedOutput\":\"rg: missing.rs: No such file or directory\\n\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert!(app.messages[0]
        .text
        .starts_with("✱ Search src/main.rs [pattern=foo]\n$ rg -n foo src/main.rs missing.rs"));
    assert!(app.messages[0]
        .text
        .contains("rg: missing.rs: No such file or directory"));
    assert!(app.messages[0].text.contains("exit code: 2"));
}

#[test]
fn exec_command_end_generic_shell_nl_sed_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_nl\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_read_nl\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"nl -ba src/main.rs | sed -n '3398,3465p'\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_nl\",\"aggregatedOutput\":\"3398 abc\\n3399 def\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/main.rs [lines=3398..3465]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_generic_shell_rg_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_rg\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"rg -n \\\"count|build\\\\(\\\" src/app/render.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg\",\"aggregatedOutput\":\"916:build_rendered_block_for_message\\n927:count_rendered_block_for_message\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app/render.rs [pattern=count|build\\(]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_generic_shell_sed_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_sed\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"sed -n '916,1015p' src/app/render.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed\",\"aggregatedOutput\":\"pub(super) fn build_rendered_block_for_message(\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app/render.rs [lines=916..1015]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_rg_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\",\"command\":\"rg -n 'bundles:|runtime:|full:|bin:|misc:' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"95:bundles:\\n102:  full:\\n111:  runtime:\\n116:  bin:\\n912:  misc:\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search pkg/core/userland/util_linux.yaml [pattern=bundles:|runtime:|full:|bin:|misc:]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_sed_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\",\"command\":\"sed -n '95,125p' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"bundles:\\n  dev:\\n  - bin\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search pkg/core/userland/util_linux.yaml [lines=95..125]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_nl_sed_pipeline_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\",\"command\":\"nl -ba /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend | sed -n '1,120p'\",\"aggregatedOutput\":\"     1\\tFILESEXTRAPATHS:prepend := \\\"${THISDIR}/files:\\\"\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend [lines=1..120]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_rg_multi_path_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\",\"command\":\"rg -n 'bash_completion|bash-completion' asm pkg\",\"aggregatedOutput\":\"asm/foo:1:...\\npkg/bar:2:...\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search asm pkg [pattern=bash_completion|bash-completion]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

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
    assert_eq!(app.messages[0].text, "← Edit src/main.rs");
    assert_eq!(app.messages[0].role, Role::ToolCall);
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
    assert!(rendered.contains("run `cargo test`"));
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
    assert_eq!(rendered, "run `ls -1`\n$ ls -1");
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

