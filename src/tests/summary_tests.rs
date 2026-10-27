//! Keep late labels scoped to their items and leave tool bodies and failures intact.

use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::notifications::handle_server_message_line;
use crate::app::transcript_render::build_rendered_lines;
use crate::app::AppState;
use crate::summaries::{command_key, SummaryEvent};

fn item() -> Value {
    json!({"type":"commandExecution","id":"command","command":"python3 process.py",
        "aggregatedOutput":"original output","exitCode":2})
}

fn notify(app: &mut AppState, method: &str) {
    let line =
        json!({"method":method,"params":{"threadId":app.thread_id,"turnId":"turn","item":item()}});
    handle_server_message_line(app, &line.to_string());
}

fn app() -> AppState {
    let mut app = AppState::new("thread".into());
    app.summaries.enabled = true;
    app.summaries.draft = true;
    app
}

#[test]
fn late_labels_preserve_command_output_failure_and_survive_duplicate_completion() {
    let mut app = app();
    notify(&mut app, "item/started");
    let visible = build_rendered_lines(&app.messages, 100);
    assert!(visible.iter().any(|l| l.text.contains("process.py")));
    notify(&mut app, "item/completed");
    let idx = app.item_to_message_index["command"];
    let original = app.messages[idx].text.clone();
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        "thread:command".into(),
        Some("Rewrite the report".into()),
    ));
    notify(&mut app, "item/completed");
    assert_eq!(app.messages[idx].text, original);
    let tool = app.messages[idx].tool_output.as_ref().expect("metadata");
    assert_eq!(tool.summary, "run Rewrite the report");
    assert!(tool.failed && tool.generated);
    let visible = build_rendered_lines(&app.messages, 100);
    assert!(visible
        .iter()
        .any(|l| l.text.contains("Rewrite the report")));
    assert!(visible.iter().any(|l| l.text.contains("original output")));
}

#[test]
fn old_results_cannot_update_a_new_thread_with_the_same_command_and_item_id() {
    let mut app = app();
    notify(&mut app, "item/completed");
    app.reset_for_forked_thread("other-thread");
    notify(&mut app, "item/completed");
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        "thread:command".into(),
        Some("Wrong old label".into()),
    ));
    assert!(
        !app.messages[0]
            .tool_output
            .as_ref()
            .expect("metadata")
            .generated
    );
}

#[test]
fn disabling_restores_the_original_label_without_changing_its_body() {
    let mut app = app();
    notify(&mut app, "item/completed");
    let original = app.messages[0]
        .tool_output
        .as_ref()
        .expect("metadata")
        .summary
        .clone();
    let body = app.messages[0].text.clone();
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        "thread:command".into(),
        Some("Rewrite the report".into()),
    ));
    app.summaries.draft = false;
    app.apply_summary_setting();
    assert_eq!(
        app.messages[0]
            .tool_output
            .as_ref()
            .expect("metadata")
            .summary,
        original
    );
    assert_eq!(app.messages[0].text, body);
}

#[test]
fn restored_labels_are_accepted_for_saved_items_without_creating_a_model_job() {
    let mut app = app();
    let response = json!({"result":{"thread":{"id":"thread","turns":[
        {"id":"turn","status":"completed","items":[item()]}]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("history");
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        "thread:command".into(),
        Some("Saved label".into()),
    ));
    assert_eq!(
        app.messages[0]
            .tool_output
            .as_ref()
            .expect("metadata")
            .summary,
        "run Saved label"
    );
}

#[test]
fn ep130_scripts_inside_ssh_receive_labels_without_reading_remote_files_locally() {
    let mut app = app();
    let script = "python3 - <<'PY'\nimport subprocess,pathlib,plistlib\nu='simulator-id'\np=pathlib.Path(subprocess.check_output(['xcrun','simctl','get_app_container',u,'com.example.app','data'],text=True).strip())\nx=plistlib.loads((p/'Library/Preferences/com.example.app.plist').read_bytes())\nprint({k:v for k,v in x.items() if k.startswith('tmdb.') and ('language' in k or 'locale' in k)})\nPY";
    let remote = format!(
        "rtk proxy ssh -o BatchMode=yes user@host {}",
        shlex::try_quote(script).expect("quote")
    );
    let command = json!({"type":"commandExecution","id":"remote","command":remote,
        "aggregatedOutput":"original remote output","exitCode":0});
    let line = json!({"method":"item/completed","params":{"threadId":"thread","item":command}});
    handle_server_message_line(&mut app, &line.to_string());
    app.handle_summary_event(SummaryEvent::Ready(
        command_key(&remote),
        "thread:remote".into(),
        Some("Read simulator preferences on the remote host".into()),
    ));
    let tool = app.messages[0].tool_output.as_ref().expect("tool metadata");
    assert!(tool.generated);
    assert!(tool.summary.contains("Read simulator preferences"));
    let command = "rtk proxy ssh user@host 'python3 process.py'";
    let line = json!({"method":"item/completed","params":{"threadId":"thread","item":{
        "type":"commandExecution","id":"remote-file","command":command,"exitCode":0}}});
    handle_server_message_line(&mut app, &line.to_string());
    app.handle_summary_event(SummaryEvent::Ready(
        command_key(command),
        "thread:remote-file".into(),
        Some("Wrong local source".into()),
    ));
    assert!(
        !app.messages[1]
            .tool_output
            .as_ref()
            .expect("metadata")
            .generated
    );
}
