//! Retain permits through queued upgrades and release unauthenticated and total quotas exactly once.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::Admissions;

#[test]
fn unfinished_logins_include_upgraded_sockets_and_authorization_releases_only_that_reserve() {
    let pool = Arc::new(Admissions::default());
    let mut admitted: Vec<_> = (0..4).map(|_| pool.admit().expect("login slot")).collect();
    assert!(pool.admit().is_none());
    admitted[0].authorize();
    admitted[0].authorize();
    assert_eq!(pool.unauthenticated.load(Ordering::Acquire), 3);
    admitted.push(pool.admit().expect("one released login slot"));
    assert_eq!(pool.total.load(Ordering::Acquire), 5);
    let queued = admitted.pop().expect("queued upgrade");
    let worker = queued.clone();
    drop(queued);
    assert!(
        pool.admit().is_none(),
        "a live worker still owns its unfinished login"
    );
    drop(worker);
    assert_eq!(pool.unauthenticated.load(Ordering::Acquire), 3);
    drop(admitted);
    assert_eq!(pool.total.load(Ordering::Acquire), 0);
    assert_eq!(pool.unauthenticated.load(Ordering::Acquire), 0);
}

#[test]
fn authenticated_connections_keep_total_slots_and_concurrent_authorization_cannot_underflow() {
    let pool = Arc::new(Admissions::default());
    let mut sockets = Vec::new();
    for _ in 0..crate::limits::NETWORK_PEERS {
        let admission = pool.admit().expect("network slot");
        admission.authorize();
        sockets.push(admission);
    }
    assert!(pool.admit().is_none());
    let admission = sockets.pop().expect("last socket");
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let admission = admission.clone();
            std::thread::spawn(move || admission.authorize())
        })
        .collect();
    for worker in workers {
        worker.join().expect("authorize");
    }
    assert_eq!(pool.unauthenticated.load(Ordering::Acquire), 0);
    drop(admission);
    assert!(pool.admit().is_some());
    drop(sockets);
    assert_eq!(pool.total.load(Ordering::Acquire), 0);
    assert_eq!(pool.unauthenticated.load(Ordering::Acquire), 0);
}
