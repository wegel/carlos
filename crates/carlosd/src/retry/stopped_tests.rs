//! Preserve accepted writes and recover only exact unconfirmed commands from both clients.

use std::time::Instant;

use carlos_proto::commands::{Command, SendPolicy, SessionOperation};
use carlos_proto::ids::{ClientId, OperationId, SessionId};
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::{Admission, Class, RetryStore};

#[test]
fn stop_recovers_each_clients_exact_pending_text_and_excludes_its_waiter() {
    let now = Instant::now();
    let session: SessionId = "b".repeat(32).parse().expect("session");
    let mut store = RetryStore::new("a".repeat(32).parse().expect("daemon"));
    let first = store.hello(None, now).expect("first").client;
    let second = store.hello(None, now).expect("second").client;
    for (client, text) in [(&first, "first private 🦊"), (&second, "second private 🐙")] {
        let operation = operation(client, 1);
        let command = Command::Send {
            session: session.clone(),
            operation: operation.clone(),
            text: text.into(),
            policy: SendPolicy::Queue,
        };
        retain(
            &mut store,
            client,
            operation,
            &session,
            &command,
            Class::Ordinary,
        );
    }
    let stop = operation(&second, 2);
    let command = Command::Stop(SessionOperation {
        session: session.clone(),
        operation: stop.clone(),
    });
    retain(
        &mut store,
        &second,
        stop.clone(),
        &session,
        &command,
        Class::Lifecycle,
    );
    let results = store.stopped(&session, &[stop]);
    assert_eq!(results.len(), 2);
    for result in results {
        let OperationState::Uncertain {
            recovery: Some(draft),
            ..
        } = &result.state
        else {
            panic!("exact recovery");
        };
        assert_eq!(draft.operation.as_ref(), Some(&result.operation));
        assert_eq!(
            draft.text,
            if result.operation.client == first {
                "first private 🦊"
            } else {
                "second private 🐙"
            }
        );
        store.complete(result).expect("within its reserved bytes");
    }
    assert!(store.stopped(&session, &[operation(&second, 2)]).is_empty());
}

#[test]
fn full_ordinary_and_control_windows_leave_a_stop_reserve_and_do_not_change_acceptance() {
    let now = Instant::now();
    let session: SessionId = "b".repeat(32).parse().expect("session");
    let mut store = RetryStore::new("a".repeat(32).parse().expect("daemon"));
    let client = store.hello(None, now).expect("client").client;
    for class in [Class::Ordinary, Class::Control] {
        store.windows[class.index()].count_limit = 1;
    }
    let accepted = fill_windows(&mut store, &client, &session, now);
    for class in [Class::Ordinary, Class::Control] {
        assert!(store
            .admit(
                &client,
                operation(&client, 4),
                Some(session.clone()),
                b"full".to_vec(),
                class,
                now
            )
            .is_err());
    }
    assert!(store
        .admit(
            &client,
            operation(&client, 4),
            Some(session.clone()),
            b"stop".to_vec(),
            Class::Lifecycle,
            now
        )
        .is_ok());
    assert_eq!(store.stopped(&session, &[operation(&client, 4)]).len(), 2);
    // The first result was evicted to admit a new ordinary command, and its counter stays expired.
    assert!(store.read(&client, &accepted.operation, now).is_err());
}

fn operation(client: &ClientId, counter: u64) -> ClientOperation {
    ClientOperation {
        client: client.clone(),
        id: OperationId::new(counter).expect("counter"),
    }
}

fn fill_windows(
    store: &mut RetryStore,
    client: &ClientId,
    session: &SessionId,
    now: Instant,
) -> carlos_proto::submissions::OperationResult {
    let mut accepted = None;
    for (counter, class) in [
        (1, Class::Ordinary),
        (2, Class::Ordinary),
        (3, Class::Control),
    ] {
        let Admission::New(result) = store
            .admit(
                client,
                operation(client, counter),
                Some(session.clone()),
                b"command".to_vec(),
                class,
                now,
            )
            .expect("admission")
        else {
            panic!("new");
        };
        if counter == 1 {
            let result = carlos_proto::submissions::OperationResult {
                state: OperationState::Accepted {
                    entry: None,
                    notice: None,
                },
                ..result
            };
            store.complete(result.clone()).expect("accept");
            accepted = Some(result);
        }
    }
    accepted.expect("original acceptance")
}

fn retain(
    store: &mut RetryStore,
    client: &ClientId,
    operation: ClientOperation,
    session: &SessionId,
    command: &Command,
    class: Class,
) {
    store
        .admit(
            client,
            operation,
            Some(session.clone()),
            serde_json::to_vec(command).expect("typed"),
            class,
            Instant::now(),
        )
        .expect("admit");
}
