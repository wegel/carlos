//! Classify completed native shell commands without an earlier override.

use super::{feed as handle_notification_line, session};
use crate::session::models::Role;

#[test]
fn item_completed_shell_rg_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\",\"command\":\"rg -n 'bundles:|runtime:|full:|bin:|misc:' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"95:bundles:\\n102:  full:\\n111:  runtime:\\n116:  bin:\\n912:  misc:\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search pkg/core/userland/util_linux.yaml [pattern=bundles:|runtime:|full:|bin:|misc:]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_shell_semeja_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_semeja_fallback\",\"command\":\"semeja search 'grep and rg command summaries' .\",\"aggregatedOutput\":\"Search results for: grep and rg command summaries\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search . [pattern=grep and rg command summaries]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_shell_sed_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\",\"command\":\"sed -n '95,125p' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"bundles:\\n  dev:\\n  - bin\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search pkg/core/userland/util_linux.yaml [lines=95..125]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_shell_nl_sed_pipeline_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\",\"command\":\"nl -ba /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend | sed -n '1,120p'\",\"aggregatedOutput\":\"     1\\tFILESEXTRAPATHS:prepend := \\\"${THISDIR}/files:\\\"\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0].tool_output.as_ref().expect("tool details").summary,
        "✱ Search /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend [lines=1..120]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_shell_rg_multi_path_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\",\"command\":\"rg -n 'bash_completion|bash-completion' asm pkg\",\"aggregatedOutput\":\"asm/foo:1:...\\npkg/bar:2:...\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search asm pkg [pattern=bash_completion|bash-completion]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn item_completed_shell_pipeline_with_empty_rhs_does_not_panic() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bad_pipe\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bad_pipe\",\"command\":\"cat src/main.rs |\",\"aggregatedOutput\":\"\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
    assert!(app.messages()[0].text.starts_with("$ cat src/main.rs |"));
    assert!(app.messages()[0].text.contains("exit code: 2"));
}

#[test]
fn item_completed_shell_bare_pipe_does_not_panic() {
    let mut app = session("thread-1");
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bare_pipe\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_bare_pipe\",\"command\":\"|\",\"aggregatedOutput\":\"\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
    assert!(app.messages()[0].text.starts_with("$ |"));
    assert!(app.messages()[0].text.contains("exit code: 2"));
}
