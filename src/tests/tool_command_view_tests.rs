//! Shorten only safe directory guards while preserving native bytes and scoped label identity.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::models::Role;
use crate::app::notifications::handle_server_message_line;
use crate::app::summary_request::summary_input;
use crate::app::tool_command_view::ToolCommandView;
use crate::app::AppState;
use crate::summaries::{command_key, SummaryEvent};

fn app() -> AppState {
    let mut app = AppState::new("thread".into());
    app.summaries.cwd = PathBuf::from("/workspace/project");
    app.summaries.enabled = true;
    app
}

fn item(command: &str) -> Value {
    json!({"id":"shell","type":"commandExecution","command":command,"aggregatedOutput":"kept stdout","exitCode":0})
}

fn complete(app: &mut AppState, item: &Value) {
    let frame = json!({"method":"item/completed","params":{"threadId":app.thread_id,"turnId":"turn","item":item}});
    handle_server_message_line(app, &frame.to_string());
}

#[test]
fn redundant_prefix_disappears_from_display_but_native_command_and_stdout_stay_exact() {
    let app = app();
    let command = "cd -- '/workspace/project'&&rtk proxy pytest";
    let mut native = item(command);
    native["aggregatedOutput"] = json!(format!("stdout quotes the original: {command}"));
    let before = native.clone();
    let output = app
        .format_tool_item(&native, Role::ToolOutput)
        .expect("display");
    assert!(output.starts_with("$ rtk proxy pytest\n"));
    assert!(
        output.contains(command),
        "stdout is never searched/replaced"
    );
    assert!(!output.contains("(in "));
    assert_eq!(native, before);
}

#[test]
fn only_structurally_marked_claude_echo_is_rewritten_once() {
    let app = app();
    let command = "cd /workspace/project && printf done";
    let output = format!("$ {command}\nstdout: $ {command}\nexit code: 0");
    let mut native = json!({"type":"toolResult","tool":"Bash","command":command,"output":output});
    assert_eq!(
        app.format_tool_item(&native, Role::ToolOutput)
            .expect("unmarked"),
        output
    );
    native["carlosCommandEcho"] = json!(true);
    let displayed = app
        .format_tool_item(&native, Role::ToolOutput)
        .expect("marked echo");
    assert!(displayed.starts_with("$ printf done\n"));
    assert!(displayed.contains(&format!("stdout: $ {command}")));
    assert_eq!(native["command"], command);
    assert_eq!(native["output"], output);
}

#[test]
fn relative_targets_need_command_cwd_and_parent_components_never_guess_logical_cd() {
    let root = Path::new("/workspace/project");
    assert!(
        ToolCommandView::from_item(&item("cd ./scripts && python3 process.py"), root).is_none()
    );
    let mut native = item("cd ./scripts && python3 process.py");
    native["cwd"] = json!(root);
    let view = ToolCommandView::from_item(&native, root).expect("explicit starting directory");
    assert_eq!(view.cwd, root.join("scripts"));
    assert_eq!(view.directory.as_deref(), Some("scripts"));
    for command in [
        "cd scripts && python3 process.py",
        "cd ../scripts && python3 process.py",
        "cd /workspace/link/../scripts && python3 process.py",
    ] {
        native["command"] = json!(command);
        assert!(
            ToolCommandView::from_item(&native, root).is_none(),
            "{command}"
        );
    }
}

#[test]
fn literal_shell_launchers_keep_exact_bodies_and_do_not_strip_unicode_whitespace() {
    let app = app();
    let body = "python3 - <<'PY'\nprint('cd /literal && text')\nPY\n";
    let script = format!("cd /workspace/project && {body}");
    for native in [
        item(&format!(
            "bash -lc {}",
            shlex::try_quote(&script).expect("quote")
        )),
        json!({"command":["/bin/bash","-lc",script]}),
    ] {
        let view =
            ToolCommandView::from_item(&native, &app.summaries.cwd).expect("literal launcher");
        assert_eq!(view.command, body);
    }
    assert!(ToolCommandView::from_item(
        &item("\u{a0}cd /workspace/project && python3 process.py"),
        &app.summaries.cwd
    )
    .is_none());
    let view = ToolCommandView::from_item(
        &item("cd /workspace/project && \u{a0}program"),
        &app.summaries.cwd,
    )
    .expect("prefix");
    assert_eq!(view.command, "\u{a0}program");
    let call = app
        .format_tool_item(
            &item("cd /workspace/project && \u{a0}ssh host 'printf text'"),
            Role::ToolCall,
        )
        .expect("literal NBSP program");
    assert!(call.contains("$ \u{a0}ssh host 'printf text'"));
    assert!(!call.contains("remote exec"));
    let displayed = app
        .format_tool_item(
            &item(&format!("cd /workspace/project && {body}")),
            Role::ToolCall,
        )
        .expect("body");
    assert!(
        displayed.ends_with(&format!("$ {body}")),
        "trailing heredoc newline remains"
    );
    let inner = "bash -lc 'ssh host true'";
    assert!(app
        .format_tool_item(
            &item(&format!("cd /workspace/project && {inner}")),
            Role::ToolCall
        )
        .expect("inner shell")
        .ends_with(&format!("$ {inner}")));
}

#[test]
fn multiline_and_remote_labels_keep_directory_in_the_first_visible_line() {
    let app = app();
    for body in ["python3 - <<'PY'\nprint(1)\nPY", "ssh host 'printf remote'"] {
        let native = item(&format!("cd /workspace/project/scripts && {body}"));
        let displayed = app
            .format_tool_item(&native, Role::ToolCall)
            .expect("display");
        assert!(displayed
            .lines()
            .next()
            .expect("label")
            .contains("(in scripts)"));
        assert!(displayed.contains(&format!(
            "$ {}",
            if body.starts_with("ssh") {
                "printf remote"
            } else {
                body
            }
        )));
    }
}

#[test]
fn long_prefix_cannot_force_a_trivial_read_into_ai_or_merge_different_directory_reads() {
    let mut app = app();
    for directory in [
        "a/very/long/parent/name/one/shared",
        "a/very/long/parent/name/two/shared",
        "one [a]",
        "one [b]",
    ] {
        let native = item(&format!(
            "cd '/workspace/project/{directory}' && rtk read file.rs"
        ));
        assert!(summary_input(&native, &app.summaries.cwd).is_none());
        let label = app.tool_command_summary(&native).expect("read label");
        assert!(label.contains(directory));
        app.append_message(Role::ToolCall, label);
    }
    assert!(!app.messages[0].text.is_empty());
    assert!(!app.messages[1].text.is_empty());
    assert_ne!(app.messages[0].text, app.messages[1].text);
    assert!(app.messages[2].text.contains("one [a]"));
    assert!(app.messages[3].text.contains("one [b]"));
}

#[test]
fn multiline_builtin_search_keeps_directory_in_its_compact_first_line() {
    let mut app = app();
    let native = item("cd /workspace/project/scripts && rg 'a\nb' file");
    let label = app.tool_command_summary(&native).expect("builtin search");
    assert!(label
        .lines()
        .next()
        .expect("first line")
        .contains("(in scripts)"));
    app.append_message(Role::ToolCall, "fallback");
    app.describe_tool_output(0, &native);
    assert!(app.messages[0]
        .tool_output
        .as_ref()
        .expect("compact label")
        .summary
        .contains("(in scripts)"));
}

#[test]
fn long_cd_cannot_spend_an_ai_request_on_a_short_cat_head_pipeline() {
    let root = format!("/workspace/{}", "long-project-directory/".repeat(12));
    let body = "cat file | head -150";
    let command = format!("cd '{}' && {body}", root.trim_end_matches('/'));
    assert!(command.len() >= 200);
    assert!(crate::summaries::eligibility::eligible(&command));
    let native = item(&command);
    let view = ToolCommandView::from_item(&native, Path::new(&root)).expect("prefix");
    assert_eq!(view.command, body);
    assert!(body.len() < 200);
    assert!(!crate::summaries::eligibility::eligible(body));
    assert!(summary_input(&native, Path::new(&root)).is_none());
}

#[test]
fn normalized_ai_inputs_scope_raw_command_version_directory_and_read_authority() {
    let root = Path::new("/workspace/project");
    let command = "cd /workspace/project/scripts && python3 process.py";
    let original = item(command);
    let first = summary_input(&original, root).expect("script input");
    assert_eq!(first.command, "python3 process.py");
    assert_eq!(first.location.directory.as_deref(), Some("scripts"));
    assert_eq!(first.location.cwd, root.join("scripts"));
    assert_eq!(first.location.read_root, root);
    assert_ne!(first.key, command_key(command));
    let mut changed = original.clone();
    changed["cwd"] = json!(root.join("other"));
    assert_ne!(
        first.key,
        summary_input(&changed, root).expect("new authority").key
    );
    changed["command"] = json!("cd /workspace/project/other && python3 process.py");
    assert_ne!(
        first.key,
        summary_input(&changed, root)
            .expect("same body elsewhere")
            .key
    );
    assert_eq!(original["command"], command);
}

#[test]
fn late_cwd_metadata_retires_generated_labels_and_old_results_even_across_off_on_toggle() {
    let mut app = app();
    let mut native = item("cd /workspace/project/scripts && python3 process.py");
    let old = summary_input(&native, &app.summaries.cwd)
        .expect("old request")
        .key;
    complete(&mut app, &native);
    app.handle_summary_event(SummaryEvent::Ready(
        old,
        "thread:shell".into(),
        Some("Old directory label".into()),
    ));
    assert!(
        app.messages[0]
            .tool_output
            .as_ref()
            .expect("label")
            .generated
    );
    native["cwd"] = json!("/workspace/project/other");
    let new = summary_input(&native, &app.summaries.cwd)
        .expect("new request")
        .key;
    complete(&mut app, &native);
    assert!(
        !app.messages[0]
            .tool_output
            .as_ref()
            .expect("new fallback")
            .generated
    );
    app.summaries.draft = false;
    app.apply_summary_setting();
    assert!(!app.messages[0]
        .tool_output
        .as_ref()
        .expect("AI off")
        .summary
        .contains("Old directory"));
    app.handle_summary_event(SummaryEvent::Ready(
        old,
        "thread:shell".into(),
        Some("Stale result".into()),
    ));
    app.handle_summary_event(SummaryEvent::Ready(
        new,
        "thread:shell".into(),
        Some("Fresh directory label".into()),
    ));
    app.summaries.draft = true;
    app.apply_summary_setting();
    let label = &app.messages[0].tool_output.as_ref().expect("AI on").summary;
    assert!(label.contains("Fresh directory label") && label.contains("(in scripts)"));
    assert!(!label.contains("Old directory") && !label.contains("Stale result"));
}

#[test]
fn history_replacement_uses_known_workspace_before_formatting_and_only_restores_cached_labels() {
    let mut app = app();
    let native = item("cd /workspace/project && python3 process.py");
    let key = summary_input(&native, &app.summaries.cwd)
        .expect("label identity")
        .key;
    let frame = json!({"result":{"thread":{"id":"thread","turns":[{"id":"past","status":"completed","items":[native]}]}}});
    load_history_from_start_or_resume(&mut app, &frame.to_string()).expect("history");
    assert!(!app.messages[0].text.contains("cd /workspace"));
    assert!(!app.messages[0].text.contains("(in "));
    assert_eq!(app.summaries.cwd, Path::new("/workspace/project"));
    app.handle_summary_event(SummaryEvent::Ready(
        key,
        "thread:shell".into(),
        Some("Saved snapshot".into()),
    ));
    assert_eq!(
        app.messages[0]
            .tool_output
            .as_ref()
            .expect("cached")
            .summary,
        "run Saved snapshot"
    );
}
