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
fn exec_command_end_generic_shell_semeja_search_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_semeja\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"semeja search \\\"grep and rg command summaries\\\" src/app\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja\",\"aggregatedOutput\":\"Search results for: grep and rg command summaries\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app [pattern=grep and rg command summaries]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_generic_shell_semeja_related_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_related_semeja\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_related_semeja\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"semeja find-related /repo/src/app/tool_shell.rs 372\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_related_semeja\",\"aggregatedOutput\":\"related snippets\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app/tool_shell.rs [line=372]"
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
fn item_completed_shell_semeja_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja_fallback\",\"command\":\"semeja search 'grep and rg command summaries' .\",\"aggregatedOutput\":\"Search results for: grep and rg command summaries\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search . [pattern=grep and rg command summaries]"
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
fn item_completed_shell_pipeline_with_empty_rhs_does_not_panic() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bad_pipe\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bad_pipe\",\"command\":\"cat src/main.rs |\",\"aggregatedOutput\":\"\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert!(app.messages[0]
        .text
        .starts_with("→ Read src/main.rs\n$ cat src/main.rs |"));
    assert!(app.messages[0].text.contains("exit code: 2"));
}

#[test]
fn item_completed_shell_bare_pipe_does_not_panic() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bare_pipe\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bare_pipe\",\"command\":\"|\",\"aggregatedOutput\":\"\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert!(app.messages[0].text.starts_with("$ |"));
    assert!(app.messages[0].text.contains("exit code: 2"));
}
