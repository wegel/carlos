//! Sends native test fixtures through the backend owner before exercising app handlers.

use std::ops::{Deref, DerefMut};

use serde_json::Value;

use crate::app::notifications::{handle_server_message_line as apply_line, ServerRequestAction};
use crate::app::state::AppState;
use crate::claude_backend::FixtureNormalizer;

pub(super) struct NormalizedApp {
    app: AppState,
    pub(super) native: FixtureNormalizer,
}

impl NormalizedApp {
    pub(super) fn new(thread: String) -> Self {
        Self {
            app: AppState::new(thread),
            native: FixtureNormalizer::default(),
        }
    }
}

impl Deref for NormalizedApp {
    type Target = AppState;
    fn deref(&self) -> &AppState {
        &self.app
    }
}
impl DerefMut for NormalizedApp {
    fn deref_mut(&mut self) -> &mut AppState {
        &mut self.app
    }
}

pub(super) fn handle_server_message_line(
    app: &mut NormalizedApp,
    line: &str,
) -> Option<ServerRequestAction> {
    let parsed: Value = serde_json::from_str(line).expect("test JSON");
    let generation = parsed["params"]["generation"]
        .as_u64()
        .unwrap_or(app.session.claude().generation());
    if parsed["method"] == "claude/sessionChanged" && generation > app.session.claude().generation()
    {
        app.native = FixtureNormalizer::default();
    }
    if parsed["method"] == "claude/controller" && generation != app.session.claude().generation() {
        return None;
    }
    let lines = app
        .native
        .lines(&parsed, &app.app.session.thread_id(), generation);
    let mut action = None;
    for line in lines {
        if let Some(next) = apply_line(&mut app.app, &line) {
            action = Some(next);
        }
    }
    action
}
