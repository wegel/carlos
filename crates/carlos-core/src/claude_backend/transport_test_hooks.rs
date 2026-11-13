//! Places deterministic test barriers at reader and delivery boundaries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Callback = Arc<dyn Fn() + Send + Sync>;

/// Shares test-only callbacks without holding the callback registry while running them.
#[derive(Default)]
pub(super) struct TestHooks(Mutex<HashMap<&'static str, Callback>>);

impl TestHooks {
    /// Install a checkpoint before starting the peer workflow.
    pub(super) fn set(&self, name: &'static str, callback: impl Fn() + Send + Sync + 'static) {
        self.0
            .lock()
            .expect("test hooks")
            .insert(name, Arc::new(callback));
    }

    /// Run a checkpoint outside all test-hook locks.
    pub(super) fn reach(&self, name: &'static str) {
        let callback = self.0.lock().expect("test hooks").get(name).cloned();
        if let Some(callback) = callback {
            callback();
        }
    }
}
