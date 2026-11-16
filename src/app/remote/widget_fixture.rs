//! Feed the real device accepted plain facts without opening sockets or reading user settings.

use carlos_client::codec::Incoming;
use carlos_client::{Client, ConnectionId, Outbound};
use carlos_core::perf_fixture::PerfRow;
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, HelloResult};
use carlos_proto::ids::{ClientId, Sequence, SessionId};
use carlos_proto::session::{Availability, Backend, Phase, SessionInfo, Snapshot};
use carlos_proto::source::SourceChunk;
use serde_json::{json, Value};

use super::Device;
use crate::app::view_state::ViewState;

pub(super) fn device(rows: Vec<PerfRow>, ralph: bool) -> Device {
    let session: SessionId = "a".repeat(32).parse().expect("fixture session");
    device_with_info(rows, info(session, ralph))
}

pub(super) fn device_with_info(rows: Vec<PerfRow>, info: SessionInfo) -> Device {
    let snapshot = Snapshot {
        info,
        entries: rows.iter().map(|row| row.entry.clone()).collect(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    };
    let (mut device, _) = device_with_snapshot(snapshot);
    for row in rows {
        install_source(&mut device, row);
    }
    device
}

/// Feed a complete snapshot through a greeted client and retain its real connection token.
pub(super) fn device_with_snapshot(snapshot: Snapshot) -> (Device, ConnectionId) {
    device_with_snapshot_for_client(snapshot, "c".repeat(32).parse().expect("fixture client"))
}

/// Feed the same checked snapshot with an explicit device's private client identity.
pub(super) fn device_with_snapshot_for_client(
    snapshot: Snapshot,
    client_id: ClientId,
) -> (Device, ConnectionId) {
    let session = snapshot.info.id.clone();
    let mut client = Client::default();
    let hello = client.begin("widget-fixture".into()).expect("greet");
    reply(
        &mut client,
        &hello,
        serde_json::to_value(greeting(client_id)).expect("hello facts"),
    );
    let request = client
        .read(
            hello.connection(),
            Command::Subscribe {
                session: session.clone(),
                since: None,
            },
        )
        .expect("subscribe");
    reply(
        &mut client,
        &request,
        json!({"kind":"snapshot", "snapshot":snapshot}),
    );
    (local_device(client, session), hello.connection())
}

fn local_device(client: Client, session: SessionId) -> Device {
    Device {
        client,
        session,
        view: ViewState::new(false),
        documents: Default::default(),
        reading: Default::default(),
        goals: Default::default(),
        queue: Default::default(),
        tasks: Default::default(),
        questions: Default::default(),
        settings: Default::default(),
        status: Default::default(),
        ralph_options: Default::default(),
        history: Default::default(),
        rename: None,
        lifecycle: None,
        waiting_ready: false,
        disconnected: None,
        reconnect: None,
    }
}

/// Feed complete bytes at the current accepted snapshot sequence.
pub(super) fn install_source(device: &mut Device, row: PerfRow) {
    let Some((source, text)) = row.source else {
        return;
    };
    let token = device
        .client
        .bodies_mut()
        .open_source(
            device.session.clone(),
            row.entry.id.clone(),
            source.revision,
        )
        .expect("source lifetime");
    let sequence = device
        .client
        .session(&device.session)
        .expect("accepted snapshot")
        .sequence();
    device
        .client
        .bodies_mut()
        .apply_source(
            &token,
            SourceChunk {
                entry: row.entry.id,
                offset: ByteOffset::new(0),
                source,
                from: None,
                next: "widget-next".to_owned().try_into().expect("fixture cursor"),
                text,
                has_more: false,
                done: true,
                seq: sequence,
            },
        )
        .expect("accepted full source");
    device
        .documents
        .changed(&device.client, &device.session, &token);
}

fn reply(client: &mut Client, request: &Outbound, result: Value) {
    let _ = client
        .receive(
            request.connection(),
            Incoming::Success {
                id: request.id(),
                result,
            },
        )
        .expect("accepted fixture reply");
}

fn greeting(client: ClientId) -> HelloResult {
    HelloResult {
        identity: None,
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "fixture".into(),
        daemon_version: "fixture".into(),
        instance: "b".repeat(32).parse().expect("instance"),
        client,
        resumed: false,
        next_operation: carlos_proto::ids::OperationId::new(1),
        resume_token: "d".repeat(64).parse().expect("credential"),
    }
}

pub(super) fn info(id: SessionId, ralph: bool) -> SessionInfo {
    SessionInfo {
        id,
        name: None,
        backend: Backend::Codex,
        model: None,
        host: "fixture".into(),
        cwd: None,
        phase: Phase::Idle,
        availability: Availability::Ready,
        current_turn: None,
        ralph,
        turn_started_at: None,
        context: None,
        quota: None,
        status: None,
    }
}
