//! Exercise platform choices while carlos-client alone owns cursors, long fields and stale replies.

use std::path::{Path, PathBuf};

use carlos_client::codec::Incoming;
use carlos_client::panels::{TaskDetailTarget, TaskFieldRead, MAX_TASK_DETAIL_BYTES};
use carlos_client::{Client, Effects};
use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use carlos_proto::tasks::{TaskField, TaskKind};
use serde_json::json;

use super::wire::Wire;

pub fn run() {
    let path =
        PathBuf::from(std::env::var("CARLOS_TEST_DAEMON_SOCKET").expect("Python fixture socket"));
    let session: SessionId = std::env::var("CARLOS_TEST_SESSION")
        .expect("Python fixture session")
        .parse()
        .expect("opaque session");
    let report =
        PathBuf::from(std::env::var("CARLOS_TEST_PANEL_REPORT").expect("private proof path"));
    let mut client = Client::default();
    let mut wire = Wire::connect(&path, &mut client);
    open_session(&mut client, &mut wire, &session);
    settle_list(&mut client, &mut wire, &session);
    assert_eq!(
        client.task_panel(&session).expect("latest list").count(),
        257
    );
    select(&mut client, &mut wire, &session, 256);
    let root = read_selected(&mut client, &mut wire, &session, "native-root");
    let older = read_older(&mut client, &mut wire, &session);
    let delayed = hold_old_field(&mut client, &mut wire, &session);
    let latest = replace_with_latest(&mut client, &mut wire, &session);
    let ignored = client
        .receive(wire.connection, delayed)
        .expect("retired original window reply");
    assert!(ignored.replies.is_empty() && ignored.panels.is_empty());
    wire.send_effects(ignored);
    assert_eq!(command_text(&client, &session), latest);
    let methods = reconnect(&path, &mut client, wire, &session);
    assert!(
        client.operations().iter().next().is_none(),
        "all actions remained reads"
    );
    std::fs::write(
        report,
        serde_json::to_vec_pretty(&json!({"root_command":root,"older_command":older,
        "rejected_old_window_reply":true,"reconnected":true,"methods":methods}))
        .expect("proof JSON"),
    )
    .expect("private fixture proof");
}

fn open_session(client: &mut Client, wire: &mut Wire, session: &SessionId) {
    let subscribe = client
        .read(
            wire.connection,
            Command::Subscribe {
                session: session.clone(),
                since: None,
            },
        )
        .expect("read existing fixture session");
    wire.send(&subscribe);
    wire.until(client, |client| client.current_session_is_ready(session));
    wire.send_effects(
        client
            .open_task_panel(session, None)
            .expect("shared task list"),
    );
}

fn read_older(client: &mut Client, wire: &mut Wire, session: &SessionId) -> String {
    wire.send_effects(
        client
            .open_older_task_window(session)
            .expect("captured older window"),
    );
    settle_list(client, wire, session);
    assert_eq!(
        client
            .task_panel(session)
            .expect("older list")
            .window()
            .expect("captured header")
            .rows,
        3
    );
    select(client, wire, session, 2);
    read_selected(client, wire, session, "native-child-256")
}

fn reconnect(path: &Path, client: &mut Client, mut wire: Wire, session: &SessionId) -> Vec<String> {
    wire.send_effects(client.close_task_details(session));
    wire.send_effects(client.close_task_panel(session));
    wire.send_effects(
        client
            .open_task_panel(session, None)
            .expect("explicit reopen"),
    );
    settle_list(client, &mut wire, session);
    let old_connection = wire.connection;
    let mut methods = std::mem::take(&mut wire.methods);
    drop(wire);
    client.disconnect(old_connection);
    let mut wire = Wire::connect(path, client);
    settle_list(client, &mut wire, session);
    assert!(client.task_panel_is_current(session));
    wire.send_effects(client.close_task_panel(session));
    methods.append(&mut wire.methods);
    methods
}

fn settle_list(client: &mut Client, wire: &mut Wire, session: &SessionId) {
    wire.until(client, |client| {
        let panel = client.task_panel(session).expect("wanted task panel");
        assert!(
            panel.read_error().is_none(),
            "task read error: {:?}",
            panel.read_error()
        );
        client.task_panel_is_current(session) && !panel.is_reading()
    });
}

fn select(client: &mut Client, wire: &mut Wire, session: &SessionId, direction: isize) {
    wire.send_effects(
        client
            .move_task_panel(session, direction)
            .expect("explicit platform selection"),
    );
    settle_list(client, wire, session);
    assert_eq!(
        client
            .task_panel(session)
            .expect("selected list")
            .selected()
            .expect("available process")
            .kind,
        TaskKind::Process
    );
}

fn target(client: &Client, session: &SessionId) -> TaskDetailTarget {
    client
        .task_panel(session)
        .and_then(|panel| panel.selected_detail_target())
        .expect("checked selected task target")
}

fn open_details(client: &mut Client, wire: &mut Wire, session: &SessionId) {
    let target = target(client, session);
    wire.send_effects(
        client
            .open_task_details(session, target)
            .expect("shared selected details"),
    );
    wire.until(client, |client| {
        let detail = client.task_details(session).expect("detail reader");
        assert!(
            detail.read_error().is_none(),
            "detail error: {:?}",
            detail.read_error()
        );
        detail.value().is_some()
    });
}

fn fields(client: &mut Client, session: &SessionId) -> Effects {
    client
        .read_task_fields(
            session,
            vec![TaskFieldRead {
                field: TaskField::Command,
                bytes: MAX_TASK_DETAIL_BYTES,
            }],
        )
        .expect("complete named command")
}

fn read_selected(client: &mut Client, wire: &mut Wire, session: &SessionId, owner: &str) -> String {
    open_details(client, wire, session);
    assert_eq!(
        client
            .task_details(session)
            .expect("detail")
            .field(TaskField::Owner),
        Some(owner)
    );
    wire.send_effects(fields(client, session));
    wire.until(client, |client| {
        let detail = client.task_details(session).expect("wanted detail");
        assert!(
            detail.read_error().is_none(),
            "field error: {:?}",
            detail.read_error()
        );
        detail.value().is_some_and(|value| {
            value
                .properties
                .iter()
                .any(|property| property.field == TaskField::Command && property.complete)
        })
    });
    command_text(client, session).to_owned()
}

fn command_text<'a>(client: &'a Client, session: &SessionId) -> &'a str {
    client
        .task_details(session)
        .expect("checked detail")
        .field(TaskField::Command)
        .expect("native command")
}

fn hold_old_field(client: &mut Client, wire: &mut Wire, session: &SessionId) -> Incoming {
    wire.send_effects(client.close_task_details(session));
    open_details(client, wire, session);
    let effects = fields(client, session);
    assert_eq!(effects.outgoing.len(), 1, "one owned named read");
    let id = effects.outgoing[0].id();
    wire.send_effects(effects);
    wire.hold_reply(client, id)
}

fn replace_with_latest(client: &mut Client, wire: &mut Wire, session: &SessionId) -> String {
    wire.send_effects(client.close_task_details(session));
    wire.send_effects(
        client
            .refresh_task_panel(session)
            .expect("explicit latest view"),
    );
    settle_list(client, wire, session);
    assert!(
        client
            .task_panel(session)
            .expect("latest view")
            .selected()
            .is_none(),
        "old window cannot select a replacement native task"
    );
    wire.send_effects(
        client
            .move_task_panel(session, 1)
            .expect("explicit first current row"),
    );
    settle_list(client, wire, session);
    select(client, wire, session, 256);
    let latest = read_selected(client, wire, session, "native-root");
    assert!(client
        .task_details(session)
        .expect("current details")
        .target()
        .window
        .is_none());
    latest
}
