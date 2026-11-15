//! Hold each admitted read's mailbox permit until its exact success, error or disconnected reply.

use carlos_proto::context_defaults::{ContextDefaults, ContextPolicy};

use super::finish_read;
use crate::engine::mailbox::Mailbox;
use crate::engine::{EngineCommand, EngineReply};

#[test]
fn original_read_permit_stays_charged_until_the_worker_result_is_returned() {
    let (mailbox, requests) = Mailbox::new(1, 4096);
    let first = mailbox
        .request(EngineCommand::ContextRead)
        .expect("admit original read");
    let original = requests.recv().expect("original envelope");
    assert!(
        mailbox.request(EngineCommand::ContextRead).is_err(),
        "in-flight read still owns its permit"
    );
    finish_read(original, Ok(defaults()));
    assert!(matches!(
        first.recv().expect("original reply"),
        EngineReply::ContextDefaults(_)
    ));
    assert!(
        mailbox.request(EngineCommand::ContextRead).is_ok(),
        "completed read released its permit"
    );
}

#[test]
fn failed_and_disconnected_reads_release_the_original_permit() {
    for disconnected in [false, true] {
        let (mailbox, requests) = Mailbox::new(1, 4096);
        let first = mailbox
            .request(EngineCommand::ContextRead)
            .expect("admit original read");
        let original = requests.recv().expect("original envelope");
        if disconnected {
            drop(first);
        }
        finish_read(original, Err("read failed".into()));
        assert!(
            mailbox.request(EngineCommand::ContextRead).is_ok(),
            "failure or disconnect releases permit"
        );
    }
}

fn defaults() -> ContextDefaults {
    ContextDefaults {
        session: "11111111111111111111111111111111"
            .parse()
            .expect("logical session"),
        saved_tokens: Some(500_000),
        policy: ContextPolicy::Owned,
    }
}
