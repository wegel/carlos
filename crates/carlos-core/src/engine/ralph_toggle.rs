//! Defer keyboard-style Ralph toggles without reading a prompt on the session owner.

use carlos_proto::ralph::RalphOptions;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use crate::backend::BackendKind;
use crate::session::codex::goals;

impl Owner {
    pub(super) fn toggle_ralph(
        &mut self,
        operation: ClientOperation,
        options: Option<Box<RalphOptions>>,
    ) {
        if self.native_turn_running() {
            self.defer_ralph_toggle(operation, options);
        } else {
            let on = !self.session.ralph_enabled();
            self.set_ralph(operation, on, on.then_some(options).flatten());
        }
    }

    fn defer_ralph_toggle(
        &mut self,
        operation: ClientOperation,
        options: Option<Box<RalphOptions>>,
    ) {
        let enabling =
            !self.session.ralph_enabled() && !self.session.ralph_runtime().toggle_pending();
        let result = (|| {
            if self.native.closed {
                return Err("backend closed; Ralph remains off".to_owned());
            }
            if enabling {
                if let Some(error) = goals::ralph_error(&self.session, self.session.codex_scope()) {
                    return Err(error.to_owned());
                }
                if let Some(options) = options {
                    self.session
                        .ralph_runtime
                        .configure_request_options(*options);
                }
            }
            self.session
                .ralph_request_toggle(true)
                .map(str::to_owned)
                .map_err(|error| error.to_string())
        })();
        let state = match result {
            Ok(status) => {
                self.session.set_status(status.clone());
                OperationState::Accepted {
                    entry: None,
                    notice: Some(status),
                }
            }
            Err(error) => rejected(error),
        };
        self.results.update(&operation, state);
        self.notice();
    }

    /// Run before queued automatic input, once the original native turn and ordinary work settle.
    pub(super) fn dispatch_pending_ralph_toggle(&mut self) -> bool {
        if !self.session.ralph_runtime().toggle_pending()
            || self.native_turn_running()
            || self.native.closed
        {
            return false;
        }
        if self.session.ralph_enabled() {
            // An accepted human prompt must survive turning off automatic work.
            self.session.ralph_stop_after_failure();
            self.session.set_status("ralph off");
            self.notice();
            return false;
        }
        self.start_ralph(None, None);
        self.workers.ordinary_busy
    }

    fn native_turn_running(&self) -> bool {
        self.session.active_turn_id().is_some()
            || self.client.kind() == BackendKind::Claude
                && self.session.claude().activity().is_running()
    }
}
