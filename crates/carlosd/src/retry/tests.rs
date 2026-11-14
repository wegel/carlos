//! Check retry rights across lost replies, counter expiry, competing clients and full ordinary work.

use std::time::{Duration, Instant};

use carlos_proto::ids::{ClientId, OperationId, SessionId};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};

use super::{Admission, Class, Resume, RetryStore};

fn store() -> RetryStore {
    RetryStore::new("a".repeat(32).parse().expect("instance"))
}
fn session() -> SessionId {
    "b".repeat(32).parse().expect("session")
}
fn operation(client: &ClientId, id: u64) -> ClientOperation {
    ClientOperation {
        client: client.clone(),
        id: OperationId::new(id).expect("counter"),
    }
}
fn admit(
    store: &mut RetryStore,
    client: &ClientId,
    id: u64,
    body: &str,
    class: Class,
    now: Instant,
) -> Admission {
    store
        .admit(
            client,
            operation(client, id),
            session(),
            body.as_bytes().to_vec(),
            class,
            now,
        )
        .expect("admit")
}
fn resume(hello: &carlos_proto::commands::HelloResult) -> Resume {
    Resume {
        client: hello.client.clone(),
        secret: hello.resume_token.clone(),
        instance: hello.instance.clone(),
    }
}

#[test]
fn lost_create_reply_reuses_its_original_session_and_private_result() {
    let now = Instant::now();
    let mut store = store();
    let hello = store.hello(None, now).expect("greeting");
    let op = operation(&hello.client, 1);
    let Admission::New(first) = admit(&mut store, &hello.client, 1, "create", Class::Ordinary, now)
    else {
        panic!("new command");
    };
    store.disconnect(&hello.client, now);
    let resumed = store.hello(Some(resume(&hello)), now).expect("resume");
    assert!(resumed.resumed);
    let proposed_new_session = "c".repeat(32).parse().expect("unused proposed session");
    let retry = store
        .admit(
            &resumed.client,
            op.clone(),
            proposed_new_session,
            b"create".to_vec(),
            Class::Ordinary,
            now,
        )
        .expect("retry");
    let Admission::Existing(pending) = retry else {
        panic!("must never create twice");
    };
    assert_eq!(pending.session, first.session);
    store
        .complete(OperationResult {
            operation: op.clone(),
            session: first.session,
            state: OperationState::Accepted { entry: None },
        })
        .expect("retained result");
    assert!(matches!(
        store
            .read(&resumed.client, &op, now)
            .expect("private recovery")
            .state,
        OperationState::Accepted { .. }
    ));
    assert_eq!(resumed.next_operation.expect("next counter").get(), 2);
}

#[test]
fn changed_work_and_another_clients_query_never_gain_retry_rights() {
    let now = Instant::now();
    let mut store = store();
    let first = store.hello(None, now).expect("first");
    let other = store.hello(None, now).expect("other");
    admit(
        &mut store,
        &first.client,
        1,
        "send original",
        Class::Ordinary,
        now,
    );
    assert_eq!(
        store
            .admit(
                &first.client,
                operation(&first.client, 1),
                session(),
                b"send changed".to_vec(),
                Class::Ordinary,
                now
            )
            .err()
            .expect("conflict")
            .code,
        -32005
    );
    assert_eq!(
        store
            .read(&other.client, &operation(&first.client, 1), now)
            .expect_err("private result")
            .code,
        -32003
    );
    let mut credential = resume(&first);
    credential.secret = "0".repeat(64).parse().expect("wrong credential");
    assert_eq!(
        store
            .hello(Some(credential), now)
            .expect_err("public id cannot authorize recovery")
            .code,
        -32003
    );
}

#[test]
fn evicted_completed_outcomes_remain_expired_instead_of_executing_again() {
    let now = Instant::now();
    let mut store = store();
    store.windows[0].count_limit = 1;
    let hello = store.hello(None, now).expect("greeting");
    let Admission::New(mut first) =
        admit(&mut store, &hello.client, 1, "first", Class::Ordinary, now)
    else {
        panic!("new");
    };
    first.state = OperationState::Accepted { entry: None };
    store.complete(first).expect("complete");
    admit(&mut store, &hello.client, 2, "second", Class::Ordinary, now);
    assert_eq!(
        store
            .admit(
                &hello.client,
                operation(&hello.client, 1),
                session(),
                b"first".to_vec(),
                Class::Ordinary,
                now
            )
            .err()
            .expect("expired retry")
            .code,
        -32006
    );
}

#[test]
fn pending_ordinary_work_cannot_consume_control_result_space_or_block_reads() {
    let now = Instant::now();
    let mut store = store();
    store.windows[0].count_limit = 1;
    let hello = store.hello(None, now).expect("greeting");
    admit(
        &mut store,
        &hello.client,
        1,
        "blocked native send",
        Class::Ordinary,
        now,
    );
    assert_eq!(
        store
            .admit(
                &hello.client,
                operation(&hello.client, 2),
                session(),
                b"second".to_vec(),
                Class::Ordinary,
                now
            )
            .err()
            .expect("ordinary full")
            .code,
        -32010
    );
    admit(
        &mut store,
        &hello.client,
        2,
        "interrupt",
        Class::Control,
        now,
    );
    assert!(store
        .read(&hello.client, &operation(&hello.client, 1), now)
        .is_ok());
}

#[test]
fn expired_inactive_id_gets_new_rights_and_cannot_recover_its_old_pending_command() {
    let now = Instant::now();
    let mut store = store();
    store.clients.lease = Duration::from_secs(1);
    let hello = store.hello(None, now).expect("greeting");
    admit(
        &mut store,
        &hello.client,
        1,
        "pending",
        Class::Ordinary,
        now,
    );
    store.disconnect(&hello.client, now);
    let next = store
        .hello(Some(resume(&hello)), now + Duration::from_secs(2))
        .expect("fresh identity");
    assert!(!next.resumed);
    assert_ne!(next.client, hello.client);
    assert!(store
        .read(&next.client, &operation(&hello.client, 1), now)
        .is_err());
}

#[test]
fn client_capacity_evicts_only_inactive_ids_and_never_recreates_their_high_water_mark() {
    let now = Instant::now();
    let mut store = store();
    store.clients.capacity = 1;
    let hello = store.hello(None, now).expect("greeting");
    assert_eq!(
        store
            .hello(None, now)
            .expect_err("active identity remains")
            .code,
        -32010
    );
    store.disconnect(&hello.client, now);
    let next = store
        .hello(
            Some(Resume {
                instance: "c".repeat(32).parse().expect("old instance"),
                ..resume(&hello)
            }),
            now,
        )
        .expect("new daemon rights");
    assert!(!next.resumed);
    assert_ne!(next.client, hello.client);
}

#[test]
fn exhausted_counter_preserves_reads_and_never_wraps_to_one() {
    let now = Instant::now();
    let mut store = store();
    let hello = store.hello(None, now).expect("greeting");
    admit(
        &mut store,
        &hello.client,
        u64::MAX,
        "last",
        Class::Control,
        now,
    );
    store.disconnect(&hello.client, now);
    let resumed = store
        .hello(Some(resume(&hello)), now)
        .expect("read-only resumed identity");
    assert!(resumed.next_operation.is_none());
    assert!(store
        .read(&resumed.client, &operation(&resumed.client, u64::MAX), now)
        .is_ok());
    assert_eq!(
        store
            .admit(
                &resumed.client,
                operation(&resumed.client, 1),
                session(),
                b"new".to_vec(),
                Class::Control,
                now
            )
            .err()
            .expect("cannot wrap")
            .code,
        -32006
    );
}
