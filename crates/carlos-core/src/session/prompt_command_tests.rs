//! Check prompt identity, exact fork anchors and draft recovery without a view.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::claude::receive_update::{ClaudeDraftContext, ClaudeEffect};
use super::prompt_commands::{PromptContext, PromptOptions};
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind, EventReceiver, PromptOrigin, RewindForkRequest};
use crate::session::models::Role;

struct Peer {
    kind: BackendKind,
    calls: Mutex<Vec<(String, Value)>>,
    forks: Mutex<Vec<RewindForkRequest>>,
    optional: Mutex<Vec<String>>,
}

impl Peer {
    fn new(kind: BackendKind) -> Self {
        Self {
            kind,
            calls: Mutex::default(),
            forks: Mutex::default(),
            optional: Mutex::default(),
        }
    }
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.calls
            .lock()
            .expect("calls")
            .push((method.into(), params));
        bail!("fixed send failure")
    }
    fn fork_from_rewind(&self, _: &str, request: RewindForkRequest, _: Duration) -> Result<String> {
        self.forks.lock().expect("forks").push(request);
        bail!("fixed fork failure")
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("unexpected reply")
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.optional
            .lock()
            .expect("optional reads")
            .push("codex".into());
        Ok(Some("quota-read".into()))
    }
    fn request_native_control(&self, method: &str, _: Value) -> Result<()> {
        self.optional
            .lock()
            .expect("optional reads")
            .push(method.into());
        Ok(())
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unexpected reply")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("no reader")
    }
    fn stop(&mut self) {}
}

fn context() -> PromptContext<'static> {
    PromptContext {
        draft: ClaudeDraftContext {
            composer: &[],
            confirmation: None,
        },
        claude_queue: false,
        edit_failed: false,
    }
}

fn session() -> SessionState {
    SessionState::new("root".into(), Default::default(), false)
}

#[test]
fn pending_prompt_keeps_its_id_when_native_echo_binds_a_row() {
    let mut state = session();
    state.record_prompt_history("original", None);
    let pending = state.input_history().prompt_id(0).expect("pending id");
    assert_eq!(state.input_history().entry_id(0), None);
    assert!(state.messages().is_empty());
    let index = state.append_message(Role::User, "original");
    let row = state.messages()[index].id;
    state.record_prompt_history("original", Some(row));
    assert_eq!(state.input_history().prompt_id(0), Some(pending));
    assert_eq!(state.input_history().entry_id(0), Some(row));
    assert_ne!(pending, row);
    state.record_prompt_turn(row, "native-turn");
    let peer = Peer::new(BackendKind::Codex);
    state.submit_rewind_text(&peer, pending, "edited".into(), &context(), false);
    assert_eq!(
        peer.forks.lock().expect("forks")[0]
            .before_turn_id
            .as_deref(),
        Some("native-turn")
    );
}

#[test]
fn pending_prompts_do_not_force_later_rows_into_the_exception_index() {
    let mut state = session();
    let first = state.transcript_append(crate::session::models::EntryDraft::plain(
        Role::User,
        "first",
    ));
    for _ in 0..100 {
        state.record_prompt_history("pending", None);
    }
    let next = state.transcript_append(crate::session::models::EntryDraft::plain(
        Role::User,
        "next",
    ));
    // Row counters resolve directly; accepted pending records must not add gaps.
    assert_eq!(next.offset_from(first), Some(1));
    for index in 0..100 {
        let prompt = state
            .input_history()
            .prompt_id(index)
            .expect("pending record");
        assert_eq!(prompt.offset_from(first), None);
        assert_eq!(state.messages().position(prompt), None);
    }
    assert_eq!(state.messages().position(next), Some(1));
}

#[test]
fn discarded_prompt_id_cannot_select_a_later_identical_draft() {
    let mut state = session();
    state.record_prompt_history("same", None);
    let retired = state.input_history().prompt_id(0).expect("first id");
    assert!(state.discard_pending_prompt("same"));
    state.record_prompt_history("same", None);
    assert_ne!(state.input_history().prompt_id(0), Some(retired));
    let peer = Peer::new(BackendKind::Grok);
    let update = state.submit_rewind_text(&peer, retired, "edit".into(), &context(), false);
    assert!(update
        .hint
        .expect("stale choice")
        .contains("no longer available"));
    assert!(peer.forks.lock().expect("forks").is_empty());
    assert!(peer.calls.lock().expect("calls").is_empty());
}

#[test]
fn unbound_prompt_uses_its_owner_record_without_inventing_a_row_or_turn() {
    let mut state = session();
    state.record_prompt_history("first", None);
    state.record_prompt_history("pending", None);
    let selected = state.input_history().prompt_id(1).expect("pending id");
    let peer = Peer::new(BackendKind::Grok);
    state.submit_rewind_text(&peer, selected, "edit".into(), &context(), false);
    assert_eq!(
        *peer.forks.lock().expect("forks"),
        [RewindForkRequest {
            keep_turns: 1,
            drop_turns: 1,
            before_turn_id: None,
            before_user_item: None
        }]
    );
    assert_eq!(state.input_history().entry_id(1), None);
}

#[test]
fn failed_send_returns_the_exact_draft_and_removes_only_its_pending_record() {
    let mut state = session();
    state.record_prompt_history("prior", None);
    state.record_prompt_history("new\ntext", None);
    let peer = Peer::new(BackendKind::Codex);
    let update = state.submit_turn_text(
        &peer,
        "new\ntext".into(),
        PromptOptions {
            record_history: true,
            force_start: false,
            origin: PromptOrigin::Human,
        },
        &context(),
    );
    assert_eq!(state.input_history().history_len(), 1);
    assert_eq!(state.input_history().text(0), "prior");
    assert!(update
        .changes
        .effects
        .iter()
        .any(|effect| matches!(effect, ClaudeEffect::SetInput(text) if text == "new\ntext")));
    let calls = peer.calls.lock().expect("calls");
    assert_eq!(calls[0].0, "turn/start");
    assert_eq!(
        calls[0].1["input"],
        json!([{"type":"text","text":"new\ntext","text_elements":[]} ])
    );
}

#[test]
fn client_events_keep_a_due_human_prompt_queued_until_the_next_idle_cursor() {
    let mut state = session();
    state.queue_turn_input_with_history("human", true);
    let now = Instant::now();
    assert!(state
        .take_dispatchable_prompt(BackendKind::Grok, false, now)
        .is_none());
    let prompt = state
        .take_dispatchable_prompt(BackendKind::Grok, true, now)
        .expect("due prompt");
    assert_eq!(prompt.text, "human");
    assert_eq!(prompt.origin, PromptOrigin::Human);
    assert!(state
        .take_dispatchable_prompt(BackendKind::Grok, true, now)
        .is_none());
}

#[test]
fn headless_quota_timers_check_the_backend_and_closed_connection_themselves() {
    for kind in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let mut state = session();
        let peer = Peer::new(kind);
        let now = Instant::now();
        state.tick_rate_limits(&peer, now);
        state.tick_claude_quota(&peer, now);
        let expected = match kind {
            BackendKind::Codex => vec!["codex"],
            BackendKind::Claude => vec!["usage.optional"],
            BackendKind::Grok => Vec::new(),
        };
        assert_eq!(*peer.optional.lock().expect("initial reads"), expected);
        state.enable_codex();
        state.close_codex_native(json!({}).as_object().expect("close facts"), "");
        state.close_claude();
        let later = now + Duration::from_secs(600);
        state.tick_rate_limits(&peer, later);
        state.tick_claude_quota(&peer, later);
        assert_eq!(*peer.optional.lock().expect("closed reads"), expected);
    }
}
