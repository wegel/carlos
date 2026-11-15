//! Preserve native command overrides when the output completes.

use super::{feed as handle_notification_line, session};
use crate::session::models::Role;

#[test]
fn exec_command_end_read_override_retains_read_output_and_summary_on_success() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "→ Read src/main.rs"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_search_override_retains_output_and_summary_on_success() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/main.rs [pattern=foo]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_search_override_kept_on_error_with_output() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
    assert!(app.messages()[0]
        .text
        .starts_with("✱ Search src/main.rs [pattern=foo]\n$ rg -n foo src/main.rs missing.rs"));
    assert!(app.messages()[0]
        .text
        .contains("rg: missing.rs: No such file or directory"));
    assert!(app.messages()[0].text.contains("exit code: 2"));
}

#[test]
fn exec_command_end_generic_shell_nl_sed_is_summarized_as_search() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/main.rs [lines=3398..3465]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_rg_is_summarized_as_search() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/app/render.rs [pattern=count|build\\(]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_semeja_search_is_summarized_as_search() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/app [pattern=grep and rg command summaries]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_rtk_semeja_search_is_summarized_as_search() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rtk_semeja\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_rtk_semeja\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"rtk semeja search \\\"Cinero Android app package existing Android TV Screen app\\\" .\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rtk_semeja\",\"aggregatedOutput\":\"Search results for: Cinero Android app package existing Android TV Screen app\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search . [pattern=Cinero Android app package existing Android TV Screen app]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_semeja_related_is_summarized_as_search() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/app/tool_shell.rs [line=372]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_rtk_proxy_semeja_related_is_summarized_as_search() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_related_rtk_semeja\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_related_rtk_semeja\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"rtk proxy semeja find-related /repo/src/app/tool_shell.rs 372\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_related_rtk_semeja\",\"aggregatedOutput\":\"related snippets\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/app/tool_shell.rs [line=372]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_sed_is_summarized_as_search() {
    let mut app = session("thread-1");
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

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/app/render.rs [lines=916..1015]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}

#[test]
fn exec_command_end_generic_shell_rtk_nl_sed_is_summarized_as_search() {
    let mut app = session("thread-1");
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_rtk_nl\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_read_rtk_nl\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"rtk nl -ba src/main.rs | sed -n '1,40p'\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_rtk_nl\",\"aggregatedOutput\":\"1 abc\\n2 def\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages().len(), 1);
    assert_eq!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool details")
            .summary,
        "✱ Search src/main.rs [lines=1..40]"
    );
    assert_eq!(app.messages()[0].role, Role::ToolOutput);
}
