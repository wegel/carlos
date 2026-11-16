//! Write cancellation before retiring its turn, and never finish a newer native prompt.

use std::sync::Arc;

use anyhow::{ensure, Result};
use serde_json::json;

use super::translate::interrupt_prompt_turn;
use super::{GrokCommands, GROK_PENDING_TURN_ID};
use crate::dispatch_guard::DispatchGuard;

impl GrokCommands {
    pub(super) fn interrupt_session(
        &self,
        session: &str,
        expected: Option<&str>,
        permission: DispatchGuard,
    ) -> Result<String> {
        let epoch = self.capture_interrupt(expected, &permission)?;
        self.process.retire_callback_choices(session, epoch);
        let translation = Arc::clone(&self.translation);
        let permission = permission.and(DispatchGuard::new(move || {
            translation
                .lock()
                .is_ok_and(|state| state.prompt_epoch == epoch)
        }));
        self.process.notify_guarded(
            "session/cancel",
            json!({"sessionId":session,"cancelTrigger":"esc"}),
            permission.clone(),
        )?;
        self.process.cancel_callbacks(session, Some(epoch))?;
        let _ = self.process.notify_guarded(
            "x.ai/queue/clear",
            json!({"sessionId":session}),
            permission,
        );
        let lines = {
            let mut state = self
                .translation
                .lock()
                .map_err(|_| anyhow::anyhow!("Grok translation lock poisoned"))?;
            if state.prompt_epoch == epoch {
                interrupt_prompt_turn(&mut state).lines
            } else {
                Vec::new()
            }
        };
        for line in lines {
            self.emit(line)?;
        }
        Ok(json!({"jsonrpc":"2.0","result":{}}).to_string())
    }

    fn capture_interrupt(&self, expected: Option<&str>, permission: &DispatchGuard) -> Result<u64> {
        permission.check()?;
        let state = self
            .translation
            .lock()
            .map_err(|_| anyhow::anyhow!("Grok translation lock poisoned"))?;
        ensure!(
            expected.is_none_or(|turn| turn == GROK_PENDING_TURN_ID
                || Some(turn) == state.current_turn_id.as_deref()),
            "captured Grok turn has changed"
        );
        // First text names the native turn. The actor can still hold its pending alias;
        // the actor's permission and this prompt epoch protect that same prompt.
        // Removing only the old in-flight set under the translator lock prevents its late result
        // from publishing completion before cancel is written. New prompts acquire this lock first.
        self.process.clear_in_flight_prompts();
        Ok(state.prompt_epoch)
    }
}

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod tests;
