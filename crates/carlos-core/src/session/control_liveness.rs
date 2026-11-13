//! Retire a turn's worker permission when its owner replaces or drops that turn.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_proto::ids::TurnId;

#[derive(Debug)]
pub(super) struct LiveTurn {
    pub(super) native: String,
    pub(super) id: TurnId,
    pub(super) live: Option<Arc<AtomicBool>>,
}

impl LiveTurn {
    pub(super) fn new(native: &str) -> Self {
        Self {
            native: native.to_owned(),
            id: uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .parse()
                .expect("turn id"),
            live: None,
        }
    }
}

impl Drop for LiveTurn {
    fn drop(&mut self) {
        if let Some(live) = &self.live {
            live.store(false, Ordering::SeqCst);
        }
    }
}
