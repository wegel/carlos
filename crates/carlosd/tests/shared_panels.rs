//! Run a second Rust consumer against the existing actual-native fixture without terminal code.

#![cfg(unix)]

#[path = "shared_panels/consumer.rs"]
mod consumer;
#[path = "shared_panels/wire.rs"]
mod wire;

#[test]
#[ignore = "scripts/test.sh supplies an actual daemon/native fixture through the Python driver"]
fn actual_daemon_panels_use_shared_readers() {
    consumer::run();
}
