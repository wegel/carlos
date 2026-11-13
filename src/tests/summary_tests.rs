//! Keep late labels scoped to their items and leave tool bodies and failures intact.

use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::notifications::handle_server_message_line;
use crate::app::transcript_render::build_rendered_lines;
use crate::app::AppState;
use carlos_core::summaries::{command_key, SummaryEvent};

fn item() -> Value {
    json!({"type":"commandExecution","id":"command","command":"python3 process.py",
        "aggregatedOutput":"original output","exitCode":2})
}

fn notify(app: &mut AppState, method: &str) {
    let line = json!({"method":method,"params":{"threadId":app.session.thread_id(),"turnId":"turn","item":item()}});
    handle_server_message_line(app, &line.to_string());
}

fn app() -> AppState {
    let mut app = AppState::new("thread".into());
    app.session.set_summary_enabled_for_test(true);
    app.view.summary_enabled_draft = true;
    app
}

#[test]
fn late_labels_preserve_command_output_failure_and_survive_duplicate_completion() {
    let mut app = app();
    notify(&mut app, "item/started");
    let visible = build_rendered_lines(app.session.messages(), 100);
    assert!(visible.iter().any(|l| l.text.contains("process.py")));
    notify(&mut app, "item/completed");
    let idx = app.item_message_index("command").expect("command entry");
    let original = app.session.messages()[idx].text.clone();
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        app.summary_token_for_test("command"),
        Some("Rewrite the report".into()),
    ));
    notify(&mut app, "item/completed");
    assert_eq!(app.session.messages()[idx].text, original);
    let tool = app.session.messages()[idx]
        .tool_output
        .as_ref()
        .expect("metadata");
    assert_eq!(tool.summary, "run Rewrite the report");
    assert!(tool.failed && tool.generated);
    let visible = build_rendered_lines(app.session.messages(), 100);
    assert!(visible
        .iter()
        .any(|l| l.text.contains("Rewrite the report")));
    assert!(visible.iter().any(|l| l.text.contains("original output")));
}

#[test]
fn old_results_cannot_update_a_new_thread_with_the_same_command_and_item_id() {
    let mut app = app();
    notify(&mut app, "item/completed");
    let old_token = app.summary_token_for_test("command");
    app.reset_for_forked_thread("other-thread");
    notify(&mut app, "item/completed");
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        old_token,
        Some("Wrong old label".into()),
    ));
    assert!(
        !app.session.messages()[0]
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
    let original = app.session.messages()[0]
        .tool_output
        .as_ref()
        .expect("metadata")
        .summary
        .clone();
    let body = app.session.messages()[0].text.clone();
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        app.summary_token_for_test("command"),
        Some("Rewrite the report".into()),
    ));
    app.view.summary_enabled_draft = false;
    app.apply_summary_setting();
    assert_eq!(
        app.session.messages()[0]
            .tool_output
            .as_ref()
            .expect("metadata")
            .summary,
        original
    );
    assert_eq!(app.session.messages()[0].text, body);
}

#[test]
fn restored_labels_are_accepted_for_saved_items_without_creating_a_model_job() {
    let mut app = app();
    let response = json!({"result":{"thread":{"id":"thread","turns":[
        {"id":"turn","status":"completed","items":[item()]}]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("history");
    app.handle_summary_event(SummaryEvent::Ready(
        command_key("python3 process.py"),
        app.summary_token_for_test("command"),
        Some("Saved label".into()),
    ));
    assert_eq!(
        app.session.messages()[0]
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
        app.summary_token_for_test("remote"),
        Some("Read simulator preferences on the remote host".into()),
    ));
    let tool = app.session.messages()[0]
        .tool_output
        .as_ref()
        .expect("tool metadata");
    assert!(tool.generated);
    assert!(tool.summary.contains("Read simulator preferences"));
    let command = "rtk proxy ssh user@host 'python3 process.py'";
    let line = json!({"method":"item/completed","params":{"threadId":"thread","item":{
        "type":"commandExecution","id":"remote-file","command":command,"exitCode":0}}});
    handle_server_message_line(&mut app, &line.to_string());
    assert!(app.summary_binding_for_test("remote-file").is_none());
    app.handle_summary_event(SummaryEvent::Ready(
        command_key(command),
        "unbound remote file".into(),
        Some("Wrong local source".into()),
    ));
    assert!(
        !app.session.messages()[1]
            .tool_output
            .as_ref()
            .expect("metadata")
            .generated
    );
}

#[cfg(unix)]
mod worker {
    //! Exercise the unchanged worker with a cache written by the previous commit.

    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::{json, Value};

    use crate::app::item_history::load_history_from_start_or_resume;
    use crate::app::notifications::handle_server_message_line;
    use crate::app::state::AppState;
    use carlos_core::summaries::SummaryEvent;

    const CHILD: &str = "CARLOS_M5C_SUMMARY_TEST_ROOT";

    #[test]
    fn replacement_cancels_model_work_and_reuses_previous_cache_hits_and_misses() {
        let root = tempfile::tempdir().expect("isolated worker files");
        prepare(root.path());
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "app::tests::summary_tests::worker::isolated_summary_worker",
                "--nocapture",
            ])
            .env(CHILD, root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_STATE_HOME", root.path().join("state"))
            .output()
            .expect("isolated summary worker test");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn prepare(root: &Path) {
        let config = root.join("config/carlos");
        let state = root.join("state/carlos");
        std::fs::create_dir_all(&config).expect("config directory");
        std::fs::create_dir_all(&state).expect("state directory");
        std::fs::write(
            state.join("tool-summaries.sqlite3"),
            include_bytes!("fixtures/tool_summaries_previous.sqlite3"),
        )
        .expect("old cache");
        let provider = "import sys,socket\ns=socket.socket(socket.AF_UNIX);s.connect(sys.argv[1]);s.sendall(b'S');s.recv(1);print('Should never be saved')";
        let settings = json!({"provider":"command","executable":"python3","arguments":["-u","-c",provider,root.join("provider.sock")],"timeout_seconds":30});
        std::fs::write(config.join("tool-summaries.json"), settings.to_string())
            .expect("isolated provider");
        std::fs::write(root.join("cancelled.py"), "print(1)\n").expect("script source");
    }

    #[test]
    fn isolated_summary_worker() {
        let Some(root) = std::env::var_os(CHILD) else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let listener = UnixListener::bind(root.join("provider.sock")).expect("provider receipt");
        let mut app = AppState::new("thread".into());
        app.session.set_summary_enabled_for_test(true);
        app.session.set_summary_cwd(root.clone());
        let (tx, rx) = mpsc::channel();
        app.configure_summary_worker(tx);
        let live = json!({"method":"item/completed","params":{"threadId":"thread","item":item("live","python3 cancelled.py")}});
        handle_server_message_line(&mut app, &live.to_string());
        let (mut provider, _) = listener.accept().expect("model child reached gate");
        provider
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("watchdog");
        let mut receipt = [0];
        provider.read_exact(&mut receipt).expect("provider began");
        assert_eq!(receipt, [b'S']);
        let response = history();
        load_history_from_start_or_resume(&mut app, &response.to_string())
            .expect("replace while model is blocked");
        assert_eq!(
            provider
                .read(&mut receipt)
                .expect("cancelled provider closed"),
            0
        );
        accept_history_labels(&mut app, &rx);
        let connection =
            rusqlite::Connection::open(root.join("state/carlos/tool-summaries.sqlite3"))
                .expect("cache");
        let saved: i64 = connection
            .query_row(
                "SELECT count(*) FROM source_links WHERE entry='thread:live'",
                [],
                |row| row.get(0),
            )
            .expect("cancelled job not cached");
        assert_eq!(saved, 0);
    }

    fn item(id: &str, command: &str) -> Value {
        json!({"id":id,"type":"commandExecution","command":command,"aggregatedOutput":"kept output","exitCode":0})
    }

    fn history() -> Value {
        json!({"result":{"thread":{"id":"thread","turns":[{"id":"past","items":[
            item("cached","python3 process.py"),
            item("inline","python3 - <<'PY'\nprint(1)\nPY"),
            item("missing","python3 missing.py")
        ]}]}}})
    }

    fn accept_history_labels(app: &mut AppState, rx: &mpsc::Receiver<SummaryEvent>) {
        for expected in [Some("Saved source label"), Some("Saved inline label"), None] {
            let event = rx
                .recv_timeout(Duration::from_secs(30))
                .expect("history cache result");
            match &event {
                SummaryEvent::Ready(_, _, label) => assert_eq!(label.as_deref(), expected),
                SummaryEvent::Failed(_, error) => panic!("cache worker failed: {error}"),
            }
            app.handle_summary_event(event);
        }
        for (index, expected) in [
            (0, "run Saved source label"),
            (1, "run Saved inline label"),
            (2, "run python3 missing.py"),
        ] {
            let entry = &app.session.messages()[index];
            assert_eq!(
                entry.tool_output.as_ref().expect("tool details").summary,
                expected
            );
            assert!(entry.text.contains("kept output"));
        }
    }
}
