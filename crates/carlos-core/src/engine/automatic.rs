//! Load Ralph prompts on a worker and keep native goals as the sole competing driver.

use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::session::codex::goals;
use crate::session::ralph::RalphConfig;

impl Owner {
    pub(super) fn set_ralph(&mut self, operation: ClientOperation, on: bool) {
        if !on {
            self.session.ralph_stop_after_failure();
            self.results.update(
                &operation,
                OperationState::Accepted {
                    entry: None,
                    notice: None,
                },
            );
        } else if self.session.ralph_enabled() {
            self.results.update(
                &operation,
                OperationState::Accepted {
                    entry: None,
                    notice: None,
                },
            );
        } else if self.native.closed {
            self.results
                .update(&operation, rejected("backend closed; Ralph remains off"));
        } else if let Some(error) = goals::ralph_error(&self.session, self.session.codex_scope()) {
            self.results.update(&operation, rejected(error));
        } else {
            let load = self.session.ralph_runtime().prepare_config();
            let target = super::target::Target::capture(&self.session, self.client.kind());
            self.workers.ordinary_busy = true;
            self.workers.ordinary.submit(move || Completion::Ralph {
                operation,
                result: load().map_err(|error| error.to_string()),
                target,
            });
        }
        self.notice();
    }

    pub(super) fn finish_ralph(
        &mut self,
        operation: ClientOperation,
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
        self.results.update(
            &operation,
            result.map_or_else(rejected, |()| OperationState::Accepted {
                entry: None,
                notice: None,
            }),
        );
    }
}
