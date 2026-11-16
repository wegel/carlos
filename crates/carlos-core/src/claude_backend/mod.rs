//! Claude Agent SDK client backed by a compiled local controller and the native CLI.

mod catalogue;
mod client_controls;
mod client_prompt;
mod commands;
mod conversation;
#[cfg(test)]
mod conversation_tests;
#[cfg(test)]
mod event_audit_tests;
mod event_batch;
mod event_emitter;
#[cfg(all(test, unix))]
mod event_eof_support;
#[cfg(all(test, unix))]
mod event_eof_tests;
mod event_facts;
#[cfg(test)]
mod event_failure_tests;
#[cfg(test)]
mod event_fixture;
mod event_handler;
mod event_history;
#[cfg(test)]
mod event_history_tests;
#[cfg(test)]
mod event_interrupt_pipe_tests;
mod event_lifecycle;
#[cfg(test)]
mod event_lifecycle_tests;
mod event_live;
mod event_normalizer;
#[cfg(test)]
mod event_normalizer_tests;
#[cfg(test)]
mod event_overflow_pipe_tests;
mod event_queue;
#[cfg(any(test, feature = "test-support"))]
mod event_race_support;
#[cfg(test)]
mod event_race_tests;
#[cfg(test)]
mod event_source_tests;
mod event_sources;
mod event_terminal;
#[cfg(any(test, feature = "test-support"))]
mod event_test_support;
mod fact_transfer;
mod helper;
pub(crate) mod history_window;
#[cfg(test)]
mod history_window_tests;
#[cfg(all(test, unix))]
mod interrupt_owner_support;
#[cfg(all(test, unix))]
mod interrupt_owner_tests;
#[cfg(test)]
mod interrupt_pending_tests;
#[cfg(test)]
mod lifecycle_differential_tests;
#[cfg(test)]
mod lifecycle_reference;
#[cfg(test)]
mod lifecycle_reference_dispatch;
#[cfg(test)]
mod lifecycle_schedule_support;
mod message_visibility;
#[cfg(all(test, unix))]
mod pipe_test_support;
mod reopen;
#[cfg(test)]
mod reopen_tests;
mod sessions;
mod snapshot;
mod startup;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod thinking;
mod tool_results;
mod translate;
#[cfg(test)]
mod translation_contract_tests;
#[cfg(test)]
mod translation_identity_tests;
#[cfg(test)]
mod translation_path_tests;
mod transport;
pub(crate) use transport::ControllerFailure;
#[cfg(all(test, target_os = "linux"))]
mod transport_drop_support;
#[cfg(all(test, target_os = "linux"))]
mod transport_drop_tests;
mod transport_reader;
mod transport_state;
#[cfg(any(test, feature = "test-support"))]
mod transport_test_hooks;
#[cfg(all(test, unix))]
mod transport_tests;
mod transport_workers;
mod transport_writer;
mod types;
mod user_record;

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind, EventReceiver, PromptOrigin, PromptPriority};
use crate::backend_commands::{BackendCommands, CommandHandle};

pub(crate) use self::catalogue::visit_saved_sessions;
pub(crate) use self::fact_transfer::FactTransfer;
#[cfg(any(test, feature = "test-support"))]
pub use self::message_visibility::is_child_report_notice;
pub use self::sessions::list_claude_sessions;
#[cfg(any(test, feature = "test-support"))]
pub use self::test_support::FixtureNormalizer;
#[cfg(any(test, feature = "test-support"))]
pub use self::translate::translate_claude_line;
pub use self::types::ClaudeLaunchMode;
#[cfg(any(test, feature = "test-support"))]
pub use self::types::ClaudeTranslationState;

use self::event_queue::EventSender;
use self::transport::ControllerTransport;
use self::types::ClaudeRuntimeSettings;

/// Owns the helper transport; startup callbacks reach the UI before startup completes.
pub struct ClaudeClient {
    handler: RefCell<self::event_handler::EventHandler>,
    cwd: PathBuf,
    transport: Arc<Mutex<Arc<ControllerTransport>>>,
    session_id: String,
    runtime_settings: Arc<Mutex<ClaudeRuntimeSettings>>,
    events_tx: EventSender,
    events_rx: Option<EventReceiver>,
}

impl ClaudeClient {
    /// Report the chosen session ID before native startup completes.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Wrap a session identity for the shared UI startup parser.
    pub fn synthetic_start_response(&self, thread_id: &str, history: Option<&Value>) -> String {
        json!({ "result": { "thread": history.cloned().unwrap_or_else(|| json!({ "id": thread_id })) } }).to_string()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn helper_call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let transport = self
            .transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .clone();
        transport.call(method, params, timeout)
    }
}

impl BackendClient for ClaudeClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn native_resume_kind(&self, thread: &str) -> crate::backend::NativeResumeKind {
        self.resume_kind(thread)
    }

    fn connection_generation(&self) -> Option<u64> {
        self.transport
            .try_lock()
            .ok()
            .map(|helper| helper.generation)
    }

    fn prepare_interrupt(&self) -> crate::backend_commands::InterruptIntent {
        let mut handler = self.handler.borrow_mut();
        crate::backend_commands::InterruptIntent {
            generation: handler.generation,
            epoch: handler.epoch,
            prompts: handler.lifecycle.begin_interrupt(),
        }
    }

    fn finish_interrupt(&self, intent: crate::backend_commands::InterruptIntent, accepted: bool) {
        let mut handler = self.handler.borrow_mut();
        if handler.generation == intent.generation && handler.epoch == intent.epoch {
            handler
                .lifecycle
                .finish_interrupt(&intent.prompts, accepted);
        }
    }

    fn take_control_events(&self) -> Vec<String> {
        self.handler.borrow_mut().take_control_events()
    }

    fn defers_terminal_events(&self) -> bool {
        self.handler.borrow().defers_terminal()
    }

    fn capture_commands(&self) -> Result<CommandHandle> {
        commands::capture(self)
    }

    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        if method == "turn/interrupt" {
            return self.interrupt(timeout);
        }
        self.commands()?.call(method, params, timeout)
    }

    fn submit_prompt(
        &self,
        _method: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        self.commands()?.submit(&params, origin, priority, timeout)
    }

    fn apply_runtime_settings(&self, model: Option<&str>, effort: Option<&str>) -> Result<bool> {
        self.commands()?.apply_runtime_settings(model, effort)
    }

    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        crate::backend_commands::BackendCommands::request_native_control(
            &self.commands()?,
            method,
            params,
        )
    }

    fn refresh_context(&self) -> Result<()> {
        crate::backend_commands::BackendCommands::refresh_context(&self.commands()?)
    }

    fn handle_event(&self, line: &str) -> Option<Vec<String>> {
        let mut handler = self.handler.borrow_mut();
        let output = handler.handle(line);
        if handler.defers_terminal() {
            if let Ok(helper) = self.transport.try_lock() {
                if helper.generation == handler.generation {
                    helper.control_closed.store(true, Ordering::Release);
                }
            }
        }
        Some(output)
    }

    fn acknowledge_event(&self, length: usize) {
        self.events_tx.handled(length);
    }

    fn fork_from_rewind(
        &self,
        thread_id: &str,
        request: crate::backend::RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        self.commands()?
            .fork_from_rewind(thread_id, request, _timeout)
    }

    fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        self.commands()?.respond(request_id, result)
    }

    fn respond_error(&self, request_id: &Value, _code: i64, message: &str) -> Result<()> {
        bail!("Cannot answer native callback {request_id}: {message}")
    }

    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("Claude events already taken"))
    }

    fn stop(&mut self) {
        if let Ok(transport) = self.transport.lock() {
            transport.stop();
        }
    }
}

impl Drop for ClaudeClient {
    fn drop(&mut self) {
        if let Ok(transport) = self.transport.lock() {
            transport.close();
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
/// Build the interrupted-turn backend message used by lifecycle fixtures.
pub fn claude_interrupted_turn_notification(params: &Value) -> Result<String> {
    use anyhow::Context;

    let thread_id = params["threadId"]
        .as_str()
        .context("missing interrupt threadId")?;
    let turn_id = params["turnId"]
        .as_str()
        .context("missing interrupt turnId")?;
    Ok(
        json!({ "method": "turn/completed", "params": { "threadId": thread_id,
        "turn": { "id": turn_id, "status": "interrupted" } } })
        .to_string(),
    )
}

#[cfg(any(test, feature = "test-support"))]
pub use event_test_support::client as receiver_client;
#[cfg(test)]
mod event_tolerance_tests;

#[cfg(any(test, feature = "test-support"))]
pub mod forwarder_test_support;

#[cfg(test)]
#[path = "../tests/backend_owner_tests.rs"]
mod backend_owner_tests;

#[cfg(test)]
mod quota_context_tests;
