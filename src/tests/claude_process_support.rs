//! Record exact native controls and answers without starting a helper process.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;

use carlos_core::backend::{BackendClient, BackendKind, EventReceiver};

#[derive(Default)]
pub(super) struct Peer {
    pub(super) controls: RefCell<Vec<(String, Value)>>,
    pub(super) replies: RefCell<Vec<(Value, Value)>>,
    pub(super) fail: Cell<bool>,
    pub(super) fail_control: Cell<bool>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        panic!("no synchronous backend call is expected")
    }

    fn respond(&self, id: &Value, answer: Value) -> Result<()> {
        anyhow::ensure!(!self.fail.get(), "injected answer failure");
        self.replies.borrow_mut().push((id.clone(), answer));
        Ok(())
    }

    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        panic!("no error answer is expected")
    }

    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        self.controls
            .borrow_mut()
            .push((method.to_string(), params));
        anyhow::ensure!(!self.fail_control.get(), "injected stop failure");
        Ok(())
    }

    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        panic!("no event channel is expected")
    }

    fn stop(&mut self) {}
}
