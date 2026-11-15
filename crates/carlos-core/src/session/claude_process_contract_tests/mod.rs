//! Preserve native task, compaction and activity rules at the actual Core boundaries.

mod activity;
mod tasks;

use carlos_proto::session::Backend;
use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::backend_test_support::context;
use super::state::SessionState;
use super::task_stop_work::TaskStopWork;

fn app() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.set_claude_enabled_for_test(true);
    core.set_client_identity(Backend::Claude, "fixture".into());
    core
}

fn fact(core: &mut SessionState, method: &str, params: Value) {
    let _ = core.handle_backend_line(
        &json!({"method":method,"params":params}).to_string(),
        &context(core),
    );
}

fn operation(counter: u64) -> ClientOperation {
    ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: counter.to_string().parse().expect("operation"),
    }
}

fn stop(core: &mut SessionState, label: &str, counter: u64) -> TaskStopWork {
    let (id, run) = {
        let row = core
            .task_rows()
            .into_iter()
            .find(|row| row.native_id() == label)
            .expect("exact native fixture id");
        (row.id(core), row.run(core).expect("known native run"))
    };
    core.prepare_task_stop(operation(counter), id, run)
        .expect("captured native stop")
}
