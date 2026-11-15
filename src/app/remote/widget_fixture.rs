//! Feed the real device accepted plain facts without opening sockets or reading user settings.

use carlos_client::codec::Incoming;
use carlos_client::{Client, Outbound};
use carlos_core::perf_fixture::PerfRow;
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, HelloResult};
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::{Availability, Backend, Phase, SessionInfo, Snapshot};
use carlos_proto::source::SourceChunk;
use serde_json::{json, Value};

use super::Device;
use crate::app::view_state::ViewState;

pub(super) fn device(rows: Vec<PerfRow>, ralph: bool) -> Device {
    let session: SessionId = "a".repeat(32).parse().expect("fixture session");
    let mut client = Client::default();
    let hello = client.begin("widget-fixture".into()).expect("greet");
    reply(
        &mut client,
        &hello,
        serde_json::to_value(greeting()).expect("hello facts"),
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
    let snapshot = Snapshot {
        info: info(session.clone(), ralph),
        entries: rows.iter().map(|row| row.entry.clone()).collect(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    };
    reply(
        &mut client,
        &request,
        json!({"kind":"snapshot", "snapshot":snapshot}),
    );
    let mut device = local_device(client, session);
    for row in rows {
        install_source(&mut device, row);
    }
    device
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
        waiting_ready: false,
        reconnect: None,
    }
}

fn install_source(device: &mut Device, row: PerfRow) {
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
                seq: Sequence::new(1),
            },
        )
        .expect("accepted full source");
    device
        .documents
        .changed(&device.client, &device.session, &token);
}

fn reply(client: &mut Client, request: &Outbound, result: Value) {
    client
        .receive(
            request.connection(),
            Incoming::Success {
                id: request.id(),
                result,
            },
        )
        .expect("accepted fixture reply");
}

fn greeting() -> HelloResult {
    HelloResult {
        identity: None,
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "fixture".into(),
        daemon_version: "fixture".into(),
        instance: "b".repeat(32).parse().expect("instance"),
        client: "c".repeat(32).parse().expect("client"),
        resumed: false,
        next_operation: carlos_proto::ids::OperationId::new(1),
        resume_token: "d".repeat(64).parse().expect("credential"),
    }
}

fn info(id: SessionId, ralph: bool) -> SessionInfo {
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
