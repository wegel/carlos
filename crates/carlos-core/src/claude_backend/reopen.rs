//! Replace a closed helper only for an explicit open, retaining its exact native launch intent.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use serde_json::json;

use super::commands::Commands;
use super::sessions;
use super::transport::ControllerTransport;
use super::types::{ClaudeLaunchMode, ClaudeRuntimeSettings};
use crate::dispatch_guard::DispatchGuard;

/// Keep fresh-root provenance until the SDK confirms that this exact root has saved history.
#[derive(Clone)]
pub(super) struct LaunchPlan {
    pub(super) id: String,
    pub(super) mode: ClaudeLaunchMode,
}

impl Commands {
    pub(super) fn reopen(
        &self,
        thread: &str,
        model: Option<String>,
        effort: Option<String>,
    ) -> Result<u64> {
        self.check()?;
        let plan = self.reopen_plan(thread)?;
        let settings = ClaudeRuntimeSettings { model, effort };
        let generation = self
            .transport
            .generation
            .checked_add(1)
            .context("Claude generation exhausted")?;
        let events = self.events_tx.for_generation(generation);
        // Reap the old helper on the ordinary worker before creating its sole replacement.
        self.transport.close();
        self.check()?;
        let replacement = Arc::new(self.transport.replacement(events.clone())?);
        let plan = self.resolve_reopen_plan(&replacement, plan)?;
        let owner = self.owner.upgrade().context("Claude owner has stopped")?;
        self.install_reopen(&owner, &replacement, &plan, &settings, &events)?;
        let permission =
            replacement_permission(owner, replacement.clone(), self.permission.clone());
        if let Err(error) = sessions::begin_session_guarded(
            &replacement,
            &self.cwd,
            &plan.mode,
            &settings,
            &plan.id,
            permission,
        ) {
            let _ = events.send(json!({"method":"claude/sessionStartFailed","params":{"message":error.to_string()}}).to_string());
        }
        Ok(generation)
    }

    fn reopen_plan(&self, thread: &str) -> Result<LaunchPlan> {
        let conversation = self
            .transport
            .conversation
            .lock()
            .map_err(|_| anyhow!("Claude conversation lock poisoned"))?;
        let plan = conversation
            .launch
            .clone()
            .context("Claude has no captured native launch")?;
        ensure!(
            plan.id == thread,
            "Claude reopen belongs to another native conversation"
        );
        Ok(plan)
    }

    fn resolve_reopen_plan(
        &self,
        replacement: &ControllerTransport,
        mut plan: LaunchPlan,
    ) -> Result<LaunchPlan> {
        if matches!(plan.mode, ClaudeLaunchMode::New) {
            let saved = replacement.call_guarded(
                "sessions.info",
                json!({"cwd":self.cwd,"sessionId":plan.id}),
                Duration::from_secs(20),
                self.write_permission(false),
            )?;
            self.check()?;
            if !saved.is_null() {
                ensure!(
                    saved["sessionId"].as_str() == Some(&plan.id),
                    "Claude saved metadata belongs to another native session"
                );
                if let Some(cwd) = saved["cwd"].as_str() {
                    ensure!(
                        std::path::Path::new(cwd) == self.cwd,
                        "Claude saved metadata belongs to another working directory"
                    );
                }
                plan.mode = ClaudeLaunchMode::Resume(plan.id.clone());
            }
        }
        Ok(plan)
    }

    fn install_reopen(
        &self,
        owner: &Mutex<Arc<ControllerTransport>>,
        replacement: &Arc<ControllerTransport>,
        plan: &LaunchPlan,
        settings: &ClaudeRuntimeSettings,
        events: &super::event_queue::EventSender,
    ) -> Result<()> {
        self.check()?;
        let mut slot = owner
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?;
        self.permission.check()?;
        ensure!(
            Arc::ptr_eq(&slot, &self.transport),
            "Claude reopen helper was replaced"
        );
        let conversation = self
            .transport
            .conversation
            .lock()
            .map_err(|_| anyhow!("Claude conversation lock poisoned"))?;
        ensure!(
            conversation.epoch == self.epoch
                && conversation
                    .launch
                    .as_ref()
                    .is_some_and(|current| current.id == plan.id),
            "Claude reopen conversation changed"
        );
        events.send(
            json!({"method":"claude/sessionChanged","params":{"sessionId":plan.id,"reopen":true}})
                .to_string(),
        )?;
        *slot = replacement.clone();
        *self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))? = settings.clone();
        Ok(())
    }
}

fn replacement_permission(
    owner: Arc<Mutex<Arc<ControllerTransport>>>,
    helper: Arc<ControllerTransport>,
    permission: DispatchGuard,
) -> DispatchGuard {
    permission.and(DispatchGuard::new(move || {
        owner
            .lock()
            .is_ok_and(|current| Arc::ptr_eq(&current, &helper))
            && helper
                .conversation
                .lock()
                .is_ok_and(|conversation| conversation.epoch == 0)
    }))
}

/// Capture the chosen root before the first history request or native start can fail.
pub(super) fn record_launch(
    helper: &ControllerTransport,
    launch: &ClaudeLaunchMode,
    fresh_id: &str,
) -> Result<()> {
    let mut conversation = helper
        .conversation
        .lock()
        .map_err(|_| anyhow!("Claude conversation lock poisoned"))?;
    let id = match launch {
        ClaudeLaunchMode::New => fresh_id,
        ClaudeLaunchMode::Resume(id) => id,
    };
    conversation.expected = Some(id.to_string());
    conversation.launch = Some(LaunchPlan {
        id: id.to_string(),
        mode: launch.clone(),
    });
    Ok(())
}
