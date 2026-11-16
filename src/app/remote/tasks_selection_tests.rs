//! Keep selected task identities and private stop errors on the real daemon device.

use carlos_client::codec::Incoming;
use carlos_client::{ConnectionId, Effects};
use carlos_proto::commands::{Command, Request};
use carlos_proto::events::{Event, Notification, SessionEvent};
use carlos_proto::ids::{Sequence, TaskId};
use carlos_proto::session::{Backend, Snapshot};
use carlos_proto::task_facts::{TaskCounts, TaskFacts};
use carlos_proto::tasks::{Task, TaskKind, TaskPage};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::{json, Value};

use super::widget_fixture::{device_with_snapshot, info};
use super::Device;

fn id(index: usize) -> TaskId {
    format!("{index:064x}").parse().expect("opaque task")
}

fn device() -> (Device, ConnectionId) {
    let mut info = info("a".repeat(32).parse().expect("session"), false);
    info.backend = Backend::Claude;
    device_with_snapshot(Snapshot {
        info,
        entries: Vec::new(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    })
}

fn key(device: &mut Device, code: KeyCode) {
    assert!(device
        .task_key(None, KeyEvent::new(code, KeyModifiers::NONE))
        .expect("local task key"));
}

fn draw(device: &mut Device) -> String {
    let mut terminal = Terminal::new(TestBackend::new(90, 20)).expect("terminal");
    terminal.draw(|frame| device.draw(frame)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn accept_page(
    device: &mut Device,
    connection: ConnectionId,
    version: u64,
    rows: &[(usize, &str)],
) {
    let revision = format!("{version:064x}").parse().expect("task revision");
    let tasks = TaskFacts {
        revision,
        overview_revision: format!("{:064x}", 100).parse().expect("overview"),
        known: true,
        reading: false,
        closed: false,
        counts: TaskCounts {
            tasks: rows.len() as u32,
            active: rows.iter().filter(|(_, state)| *state == "running").count() as u32,
            ..TaskCounts::default()
        },
        native_older_available: false,
        read_error: None,
        notice: String::new(),
    };
    let mut pending = std::mem::take(&mut device.tasks.effects);
    if device.client.task_panel(&device.session).is_none() {
        pending.push(
            device
                .client
                .open_task_panel(&device.session, None)
                .expect("shared task reader"),
        );
    }
    pending.push(
        device
            .client
            .receive(
                connection,
                Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                    session: device.session.clone(),
                    seq: Sequence::new(version),
                    event: Event::TasksChanged { tasks: Some(tasks) },
                }))),
            )
            .expect("accept numbered task facts"),
    );
    let page = task_page(device, version, rows);
    answer_task_reads(device, pending, page);
    device.tasks.paint_dirty.set(true);
}

fn task_page(device: &Device, version: u64, rows: &[(usize, &str)]) -> TaskPage {
    TaskPage {
        session: device.session.clone(),
        seq: Sequence::new(version),
        revision: format!("{version:064x}").parse().expect("task revision"),
        known: true,
        reading: false,
        closed: false,
        tasks: rows
            .iter()
            .map(|(index, state)| Task {
                id: id(*index),
                kind: TaskKind::Task,
                label: format!("task-{index}"),
                label_complete: true,
                state: (*state).into(),
                run: None,
            })
            .collect(),
        has_more: false,
        native_older_available: false,
        read_error: None,
    }
}

fn answer_task_reads(device: &mut Device, pending: Vec<Effects>, page: TaskPage) {
    let mut pending = pending;
    while let Some(effects) = pending.pop() {
        for outgoing in effects.outgoing {
            let request: Request =
                serde_json::from_slice(outgoing.bytes()).expect("typed task request");
            let result = match request.command {
                Command::TaskSubscribe { .. } => json!({"session":device.session,"seq":page.seq,
                    "tasks":device.client.session(&device.session).expect("session facts").tasks()}),
                Command::TaskList { .. } => serde_json::to_value(&page).expect("checked task page"),
                Command::TaskUnsubscribe { .. } => Value::Null,
                _ => continue,
            };
            pending.push(
                device
                    .client
                    .receive(
                        outgoing.connection(),
                        Incoming::Success {
                            id: outgoing.id(),
                            result,
                        },
                    )
                    .expect("owned task reply"),
            );
        }
    }
}

#[test]
fn opening_keeps_the_same_task_through_completion_eviction_and_reopening() {
    let (mut device, connection) = device();
    accept_page(
        &mut device,
        connection,
        2,
        &[(1, "running"), (2, "running")],
    );
    key(&mut device, KeyCode::F(2));
    assert_eq!(device.selected_task_id().cloned(), Some(id(1)));
    accept_page(
        &mut device,
        connection,
        3,
        &[(1, "completed"), (2, "running")],
    );
    assert_eq!(device.selected_task_id().cloned(), Some(id(1)));
    accept_page(&mut device, connection, 4, &[(2, "running")]);
    assert_eq!(device.selected_task_id().cloned(), Some(id(1)));
    assert!(device.selected_task().is_none());
    assert!(draw(&mut device).contains("Selected row unavailable; use arrows"));
    key(&mut device, KeyCode::F(2));
    key(&mut device, KeyCode::F(2));
    assert_eq!(
        device.selected_task_id().cloned(),
        Some(id(1)),
        "reopening cannot retarget x"
    );
    assert!(draw(&mut device).contains("Selected row unavailable; use arrows"));
    key(&mut device, KeyCode::Down);
    assert_eq!(device.selected_task_id().cloned(), Some(id(2)));
}

#[test]
fn choosing_a_row_clears_only_its_devices_stop_error_and_scroll() {
    let (mut first, first_connection) = device();
    let (mut second, second_connection) = device();
    for (device, connection) in [
        (&mut first, first_connection),
        (&mut second, second_connection),
    ] {
        accept_page(device, connection, 2, &[(1, "running"), (2, "running")]);
        key(device, KeyCode::F(2));
        device.tasks.view.error = Some("Private exact stop rejection".into());
        device.tasks.view.scroll = 17;
    }
    first.tasks.view.scroll = 0;
    assert!(draw(&mut first).contains("Private exact stop rejection"));
    first.tasks.view.scroll = 17;
    key(&mut first, KeyCode::Down);
    assert_eq!(first.selected_task_id().cloned(), Some(id(2)));
    assert_eq!(first.tasks.view.scroll, 0);
    assert!(first.tasks.view.error.is_none());
    assert!(!draw(&mut first).contains("Private exact stop rejection"));
    assert_eq!(second.selected_task_id().cloned(), Some(id(1)));
    assert_eq!(second.tasks.view.scroll, 17);
    assert_eq!(
        second.tasks.view.error.as_deref(),
        Some("Private exact stop rejection")
    );
}

#[test]
fn native_reset_and_closed_info_keep_the_exact_selected_process_and_private_composer() {
    let (mut device, connection) = device();
    accept_page(
        &mut device,
        connection,
        2,
        &[(1, "running"), (2, "running")],
    );
    key(&mut device, KeyCode::F(2));
    device.set_composer("keep this draft");
    let snapshot = reset_snapshot(&device);
    let mut info = snapshot.info.clone();
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().expect("local socket");
    let mut link =
        super::transport::Transport::from_stream(stream, &mut carlos_client::Client::default())
            .expect("nonblocking device transport");
    link.connection = connection;
    info.availability = carlos_proto::session::Availability::Closed;
    for (sequence, event) in [
        (
            3,
            Event::Reset {
                snapshot: Box::new(snapshot),
            },
        ),
        (4, Event::InfoChanged { info }),
    ] {
        device
            .receive(
                &mut link,
                Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                    session: device.session.clone(),
                    seq: Sequence::new(sequence),
                    event,
                }))),
            )
            .expect("actual device accepts current native state");
        assert_eq!(device.selected_task_id().cloned(), Some(id(1)));
        assert_eq!(device.view.input.lines().join("\n"), "keep this draft");
        let selected = device.selected_task().expect("retained task row");
        assert_eq!(selected.id, id(1));
        assert_eq!(selected.kind, TaskKind::Task);
        assert!(device.client.operations().iter().next().is_none());
    }
}

fn reset_snapshot(device: &Device) -> Snapshot {
    let accepted = device
        .client
        .session(&device.session)
        .expect("accepted session");
    Snapshot {
        info: accepted.info().clone(),
        seq: Sequence::new(3),
        tasks: accepted.tasks().cloned(),
        entries: Vec::new(),
        approvals: Vec::new(),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
    }
}
