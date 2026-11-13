//! Expose terminal-free pipe controls to the client's event-forwarder tests.

use std::process::Command;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;

use super::event_race_support::{notify, pause_once, transport, Pause};
use super::transport::ControllerTransport;
use super::ClaudeClient;
use crate::backend::EventReceiver;

pub const DEADLINE: Duration = super::event_race_support::DEADLINE;
pub const MAX_EVENT_BYTES: usize = super::event_queue::MAX_EVENT_BYTES;

/// Keep private transport fields behind plain test commands.
#[derive(Clone)]
pub struct ForwarderHarness(Arc<ControllerTransport>);

/// Start the same real pipe fixture used by backend reader tests.
pub fn command_client(command: Command, capacity: usize) -> (ClaudeClient, EventReceiver) {
    super::event_test_support::command_client(command, capacity)
}

/// Submit the fixture's ordinary human prompt.
pub fn submit(client: &ClaudeClient) -> String {
    super::event_test_support::submit(client)
}

/// Drain the fixture's initial native frames at the handled cursor.
pub fn records(client: &ClaudeClient, receiver: &EventReceiver) {
    super::event_test_support::records(client, receiver);
}

/// Call a fixed peer method without exposing the native transport.
pub fn helper_call(client: &ClaudeClient, method: &str, params: Value) -> Result<Value> {
    client.helper_call(method, params, DEADLINE)
}

/// Inspect the handler's completion queue after the forwarder reaches EOF.
pub fn completions_empty(client: &ClaudeClient) -> bool {
    client
        .handler
        .borrow_mut()
        .lifecycle
        .completions()
        .is_empty()
}

impl ForwarderHarness {
    /// Borrow the same transport as the production reader and handler.
    pub fn new(client: &ClaudeClient) -> Self {
        Self(transport(client))
    }
    /// Call a peer while another thread observes the receiver.
    pub fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        self.0.call(method, params, timeout)
    }
    /// Send one peer control without waiting for its response.
    pub fn request(&self, method: &str, params: Value) -> Result<()> {
        self.0.request(method, params)
    }
    /// Wait for a named production reader checkpoint.
    pub fn notify(&self, name: &'static str) -> mpsc::Receiver<()> {
        notify(&self.0.events.hooks, name)
    }
    /// Pause the production reader once at a named checkpoint.
    pub fn pause(&self, name: &'static str) -> Pause {
        pause_once(&self.0.events.hooks, name)
    }
    /// Add a charged frame through the actual bounded sender.
    pub fn send(&self, line: String) -> Result<()> {
        self.0.events.send(line)
    }
    /// Release only the exact bytes that the test consumed.
    pub fn handled(&self, bytes: usize) {
        self.0.events.handled(bytes);
    }
    /// Read the actual sender's outstanding byte count.
    pub fn charged_bytes(&self) -> usize {
        self.0.events.charged_bytes()
    }
    /// Check that the reader settled every waiter.
    pub fn pending_empty(&self) -> bool {
        self.0.pending.lock().expect("requests").is_empty()
    }
    /// Check the transport's actual closed flag.
    pub fn closed(&self) -> bool {
        self.0.state.is_closed()
    }
    /// Wait for the owned helper and inspect its actual exit code.
    pub fn exit_code(&self) -> Option<i32> {
        self.0
            .child
            .lock()
            .expect("child")
            .wait()
            .expect("helper exited")
            .code()
    }
}
