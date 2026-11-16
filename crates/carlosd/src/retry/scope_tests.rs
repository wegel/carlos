//! Keep valid retry credentials private to their device and preserve the local operator's capacity.

use std::time::Instant;

use carlos_proto::commands::HelloResult;
use carlos_proto::ids::{DeviceId, OperationId};
use carlos_proto::submissions::{ClientOperation, OperationResult};

use super::{Admission, Class, ClientScope, Resume, RetryStore};

fn device(character: char) -> DeviceId {
    character.to_string().repeat(32).parse().expect("device")
}

fn resume(hello: &HelloResult) -> Resume {
    Resume {
        client: hello.client.clone(),
        secret: hello.resume_token.clone(),
        instance: hello.instance.clone(),
    }
}

fn pending(store: &mut RetryStore, hello: &HelloResult, now: Instant) -> OperationResult {
    let operation = ClientOperation {
        client: hello.client.clone(),
        id: OperationId::new(1).expect("counter"),
    };
    match store
        .admit(
            &hello.client,
            operation,
            None,
            b"private".to_vec(),
            Class::Ordinary,
            now,
        )
        .expect("admitted")
    {
        Admission::New(result) => result,
        Admission::Existing(_) => panic!("new operation"),
    }
}

#[test]
fn another_device_or_unix_peer_cannot_use_even_a_correct_private_resume_secret() {
    let now = Instant::now();
    let mut store = RetryStore::new("a".repeat(32).parse().expect("instance"));
    let scope = ClientScope::Device(device('b'));
    let hello = store.hello_in(scope.clone(), None, now).expect("device");
    let result = pending(&mut store, &hello, now);
    store.disconnect(&hello.client, now);
    for foreign in [ClientScope::Device(device('c')), ClientScope::Local] {
        let error = store
            .hello_in(foreign, Some(resume(&hello)), now)
            .expect_err("a stolen resume credential is not device authority");
        assert_eq!(error.code, -32003);
    }
    let same = store
        .hello_in(scope, Some(resume(&hello)), now)
        .expect("same device");
    assert!(same.resumed);
    assert_eq!(same.client, hello.client);
    assert_eq!(
        store
            .read(&same.client, &result.operation, now)
            .expect("private result"),
        result
    );
}

#[test]
fn network_leases_never_consume_or_evict_the_local_identity_reserve() {
    let now = Instant::now();
    let mut store = RetryStore::new("a".repeat(32).parse().expect("instance"));
    store.clients.capacity = 1;
    store.clients.network_capacity = 1;
    let network = store
        .hello_in(ClientScope::Device(device('b')), None, now)
        .expect("network");
    let local = store.hello(None, now).expect("local reserve");
    store.disconnect(&local.client, now);
    assert_eq!(
        store
            .hello_in(ClientScope::Device(device('c')), None, now)
            .expect_err("network full")
            .code,
        -32010
    );
    assert!(
        store
            .hello(Some(resume(&local)), now)
            .expect("local rights intact")
            .resumed
    );
    store.disconnect(&network.client, now);
    let replacement = store
        .hello_in(ClientScope::Device(device('c')), None, now)
        .expect("evict inactive network");
    assert_ne!(replacement.client, network.client);
    assert!(store.clients.records.contains_key(&local.client));
}

#[test]
fn revoking_one_device_forgets_its_results_without_affecting_other_devices_or_late_completions() {
    let now = Instant::now();
    let mut store = RetryStore::new("a".repeat(32).parse().expect("instance"));
    let first = store
        .hello_in(ClientScope::Device(device('b')), None, now)
        .expect("first");
    let second = store
        .hello_in(ClientScope::Device(device('b')), None, now)
        .expect("same device");
    let other = store
        .hello_in(ClientScope::Device(device('c')), None, now)
        .expect("other");
    let local = store.hello(None, now).expect("local");
    let old = pending(&mut store, &first, now);
    let retained = pending(&mut store, &other, now);
    store.revoke_device(&device('b'));
    assert!(store.touch(&first.client, now).is_err());
    assert!(store.touch(&second.client, now).is_err());
    assert!(store.read(&first.client, &old.operation, now).is_err());
    assert_eq!(store.complete(old).expect("late result ignored"), None);
    assert_eq!(
        store
            .read(&other.client, &retained.operation, now)
            .expect("other intact"),
        retained
    );
    assert!(store.touch(&local.client, now).is_ok());
}

#[test]
fn pending_session_work_cannot_consume_the_local_operator_result_reserve() {
    let now = Instant::now();
    let mut store = RetryStore::new("a".repeat(32).parse().expect("instance"));
    let local = store.hello(None, now).expect("operator");
    for (index, class) in [Class::Ordinary, Class::Control, Class::Lifecycle]
        .into_iter()
        .enumerate()
    {
        store.windows[index].count_limit = 1;
        let operation = ClientOperation {
            client: local.client.clone(),
            id: OperationId::new(index as u64 + 1).expect("counter"),
        };
        store
            .admit(&local.client, operation, None, b"held".to_vec(), class, now)
            .expect("fill session work");
    }
    let operation = ClientOperation {
        client: local.client.clone(),
        id: OperationId::new(4).expect("counter"),
    };
    assert!(store
        .admit(
            &local.client,
            operation.clone(),
            None,
            b"full".to_vec(),
            Class::Ordinary,
            now
        )
        .is_err());
    assert!(matches!(
        store
            .admit(
                &local.client,
                operation,
                None,
                b"device revoke".to_vec(),
                Class::Operator,
                now
            )
            .expect("operator reserve"),
        Admission::New(_)
    ));
}
