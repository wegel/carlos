//! Open Grok sessions on a worker while the event owner drains large native history.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{ensure, Result};
use serde_json::json;

use super::process::start_grok_process;
use super::{
    grok_model_catalog, GrokClient, GrokCommands, GrokLaunchMode, GrokRuntimeSettings,
    GrokTranslationState,
};

impl GrokClient {
    /// Start a Grok process in the chosen working directory.
    pub fn start(cwd: &Path) -> Result<Self> {
        Self::start_cancellable(cwd, &crate::launch_cancel::LaunchCancellation::default())
    }

    /// Register process cancellation before waiting for native initialization.
    pub fn start_cancellable(
        cwd: &Path,
        cancel: &crate::launch_cancel::LaunchCancellation,
    ) -> Result<Self> {
        let (events_tx, events_rx) = crate::protocol_events::EventQueue::new_waiting("Grok");
        let translation = Arc::new(Mutex::new(GrokTranslationState::default()));
        let runtime_settings = GrokRuntimeSettings::default();
        let process = start_grok_process(
            cwd,
            &runtime_settings,
            events_tx.clone(),
            Arc::clone(&translation),
            cancel,
        )?;
        let commands = Arc::new(GrokCommands {
            revision: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            opening: AtomicBool::new(false),
            cwd: cwd.to_path_buf(),
            session_id: Mutex::new(None),
            runtime_settings: Mutex::new(runtime_settings),
            models: Mutex::new(grok_model_catalog()),
            translation,
            process: process.commands(),
            events_tx,
        });
        Ok(Self {
            commands,
            process_owner: Mutex::new(process),
            events_rx: Some(events_rx),
        })
    }
}

impl GrokClient {
    /// Start exactly one native session open while its consumer handles charged replay frames.
    pub fn open_session_async(&self, mode: GrokLaunchMode) -> Result<()> {
        ensure!(
            !self.commands.opening.swap(true, Ordering::SeqCst),
            "Grok session launch already pending"
        );
        if let Ok(mut state) = self.commands.translation.lock() {
            state.publish_identity = true;
        }
        let commands = self.commands.clone();
        thread::spawn(move || {
            if let Err(error) = commands.open_and_publish(&mode) {
                commands.events_tx.terminal(json!({"method":"carlos/grok/closed","params":{"message":crate::protocol_events::bounded_diagnostic(&error.to_string())}}).to_string());
                commands.process.cancel();
            }
            commands.opening.store(false, Ordering::SeqCst);
        });
        Ok(())
    }
}

impl GrokCommands {
    fn open_and_publish(&self, mode: &GrokLaunchMode) -> Result<()> {
        let id = self.open_session(mode)?;
        for line in self.take_replay_tail() {
            self.emit(line)?;
        }
        if let Some(line) = self.drain_replay_usage_line() {
            self.emit(line)?;
        }
        let models: Vec<_> = self.available_models().into_iter().map(|model| json!({
            "model":model.model,"displayName":model.display_name,"isDefault":model.is_default,
            "defaultReasoningEffort":model.default_effort,
            "supportedReasoningEfforts":model.supported_efforts.into_iter().map(|effort| json!({"reasoningEffort":effort})).collect::<Vec<_>>()
        })).collect();
        self.emit(
            json!({"method":"carlos/grok/ready","params":{"threadId":id,"models":models}})
                .to_string(),
        )
    }
}
