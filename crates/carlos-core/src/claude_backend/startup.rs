//! Preserve a chosen fresh UUID across owner replacement without assuming a transcript exists.

use std::cell::RefCell;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use super::event_queue::EventSender;
use super::reopen::{resolve_plan, LaunchPlan};
use super::transport::ControllerTransport;
use super::types::{ClaudeLaunchMode, ClaudeRuntimeSettings};
use super::{sessions, ClaudeClient};
use crate::backend::{BackendClient, NativeResumeKind};
use crate::dispatch_guard::DispatchGuard;
use crate::launch_cancel::LaunchCancellation;

impl ClaudeClient {
    /// Start the helper and let the owner answer questions during SDK startup.
    pub fn start(cwd: &Path, launch: ClaudeLaunchMode) -> Result<Self> {
        Self::start_with_context_window(cwd, launch, None)
    }

    pub fn start_with_context_window(
        cwd: &Path,
        launch: ClaudeLaunchMode,
        value: Option<&str>,
    ) -> Result<Self> {
        Self::start_cancellable(cwd, launch, value, &LaunchCancellation::default())
    }

    /// Register cancellation before checking saved metadata or sending the first SDK start.
    pub fn start_cancellable(
        cwd: &Path,
        launch: ClaudeLaunchMode,
        value: Option<&str>,
        cancel: &LaunchCancellation,
    ) -> Result<Self> {
        cancel.check()?;
        let client = Self::prepare(cwd, &launch, value)?;
        let commands = client.capture_commands()?;
        cancel.register(move || commands.abort())?;
        let cancellation = cancel.clone();
        let permission = DispatchGuard::new(move || cancellation.check().is_ok());
        let transport = client.transport.lock().expect("prepared helper").clone();
        let mode = if matches!(launch, ClaudeLaunchMode::ReopenFresh(_)) {
            resolve_plan(
                &transport,
                cwd,
                LaunchPlan {
                    id: client.session_id.clone(),
                    mode: ClaudeLaunchMode::New,
                },
                permission.clone(),
            )?
            .mode
        } else {
            launch
        };
        sessions::begin_session_guarded(
            &transport,
            cwd,
            &mode,
            &ClaudeRuntimeSettings::default(),
            &client.session_id,
            permission,
        )?;
        Ok(client)
    }

    fn prepare(cwd: &Path, launch: &ClaudeLaunchMode, value: Option<&str>) -> Result<Self> {
        let (events_tx, events_rx) = EventSender::channel(1024);
        let session_id = match launch {
            ClaudeLaunchMode::Resume(id) | ClaudeLaunchMode::ReopenFresh(id) => id.clone(),
            ClaudeLaunchMode::New => sessions::fresh_session_id(),
        };
        let transport = Arc::new(ControllerTransport::start_with_context_window(
            events_tx.clone(),
            value,
        )?);
        Ok(Self {
            handler: RefCell::default(),
            cwd: cwd.to_path_buf(),
            transport: Arc::new(Mutex::new(transport)),
            session_id,
            runtime_settings: Arc::new(Mutex::new(ClaudeRuntimeSettings::default())),
            events_tx,
            events_rx: Some(events_rx),
        })
    }

    pub(super) fn resume_kind(&self, thread: &str) -> NativeResumeKind {
        let Ok(helper) = self.transport.lock() else {
            return NativeResumeKind::Unconfirmed;
        };
        let Ok(conversation) = helper.conversation.lock() else {
            return NativeResumeKind::Unconfirmed;
        };
        match conversation
            .launch
            .as_ref()
            .filter(|plan| plan.id == thread)
            .map(|plan| &plan.mode)
        {
            Some(ClaudeLaunchMode::New | ClaudeLaunchMode::ReopenFresh(_)) => {
                NativeResumeKind::ClaudeFresh
            }
            Some(ClaudeLaunchMode::Resume(_)) => NativeResumeKind::Saved,
            None => NativeResumeKind::Unconfirmed,
        }
    }
}
