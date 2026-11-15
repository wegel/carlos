//! Load Ralph prompts on the existing worker and keep native goals as the competing driver.

use carlos_proto::ralph::RalphOptions;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::session::codex::goals;
use crate::session::models::Role;
use crate::session::ralph::RalphConfig;

impl Owner {
    pub(super) fn set_ralph(
        &mut self,
        operation: ClientOperation,
        on: bool,
        options: Option<Box<RalphOptions>>,
    ) {
        if !on && options.is_some() {
            self.ralph_result(
                Some(operation),
                Err("prompt options require Ralph on".into()),
            );
        } else if on && self.session.ralph_enabled() {
            if options
                .as_ref()
                .is_some_and(|options| options.has_overrides())
            {
                self.ralph_result(
                    Some(operation),
                    Err("Ralph is already on; turn it off before changing prompt options".into()),
                );
            } else {
                self.session.ralph_runtime.reset_pending_toggle();
                self.ralph_result(Some(operation), Ok(()));
            }
        } else if !on {
            self.session.ralph_stop_after_failure();
            self.ralph_result(Some(operation), Ok(()));
        } else {
            self.start_ralph(Some(operation), options);
        }
        self.notice();
    }

    pub(super) fn start_ralph(
        &mut self,
        operation: Option<ClientOperation>,
        options: Option<Box<RalphOptions>>,
    ) {
        if self.native.closed {
            self.ralph_result(operation, Err("backend closed; Ralph remains off".into()));
            return;
        }
        if let Some(error) = goals::ralph_error(&self.session, self.session.codex_scope()) {
            self.ralph_result(operation, Err(error.into()));
            return;
        }
        if operation.is_some() {
            self.session.ralph_runtime.reset_pending_toggle();
        }
        if let Some(options) = options {
            self.session
                .ralph_runtime
                .configure_request_options(*options);
        }
        let load = self.session.ralph_runtime().prepare_config();
        let target = super::target::Target::capture(&self.session, self.client.kind());
        self.workers.ordinary_busy = true;
        self.workers.ordinary.submit(move || Completion::Ralph {
            operation,
            result: load().map_err(|error| error.to_string()),
            target,
        });
    }

    pub(super) fn finish_ralph(
        &mut self,
        operation: Option<ClientOperation>,
        result: Result<RalphConfig, String>,
    ) {
        let result = result.and_then(|config| {
            if self.native.closed {
                return Err("backend closed while reading Ralph prompt".into());
            }
            if let Some(error) = goals::ralph_error(&self.session, self.session.codex_scope()) {
                return Err(error.into());
            }
            self.session.ralph_runtime.enable(config);
            self.session.ralph_runtime.queue_initial_prompt();
            Ok(())
        });
        self.ralph_result(operation, result);
    }

    fn ralph_result(&mut self, operation: Option<ClientOperation>, result: Result<(), String>) {
        if let Some(operation) = operation {
            self.results.update(
                &operation,
                result.map_or_else(rejected, |()| OperationState::Accepted {
                    entry: None,
                    notice: None,
                }),
            );
        } else {
            match result {
                Ok(()) => self.session.set_status("ralph on"),
                Err(error) => {
                    self.session.ralph_stop_after_failure();
                    self.session
                        .append_message(Role::System, format!("Ralph could not start: {error}"));
                    self.session.set_status(format!("ralph: {error}"));
                }
            }
        }
        self.notice();
    }
}
