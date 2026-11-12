//! Exercise every public approval command with live, retired, and arbitrary ids.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use uuid::Uuid;

use super::state::{AppState, ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind};
use super::turn_approval::respond_to_pending_approval;
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;
use crate::session::history_identity::HistoryMode;

const SEEDS: [u64; 4] = [1, 0xcafe, 0xdeadbeef, 0x9e3779b97f4a7c15];
const OPERATIONS: u64 = 28;

struct Sequence {
    app: AppState,
    client: Peer,
    known: Vec<EntryId>,
    facts: HashMap<EntryId, ApprovalRequestDraft>,
    retired: HashSet<EntryId>,
    accepts: HashMap<EntryId, usize>,
    previous: HashSet<EntryId>,
    random: u64,
    id_classes: [usize; 5],
}

#[test]
fn twenty_thousand_public_approval_sequences_preserve_identity_and_consent() {
    run_sequences(5_000, 32);
}

#[test]
#[ignore = "larger fixed-seed approval command audit"]
fn two_hundred_thousand_long_approval_sequences() {
    run_sequences(50_000, 128);
}

fn run_sequences(per_seed: usize, steps: usize) {
    let mut coverage = [0usize; OPERATIONS as usize];
    let mut id_classes = [0usize; 5];
    let mut allows = 0;
    for seed in SEEDS {
        for index in 0..per_seed {
            let mut sequence = Sequence::new(seed.wrapping_add(index as u64 * 7919));
            for step in 0..steps {
                let random = sequence.next();
                let operation = if index == 0 {
                    step as u64 % OPERATIONS
                } else {
                    random % OPERATIONS
                };
                coverage[operation as usize] += 1;
                sequence.apply(operation, random);
                sequence.check();
            }
            for (total, count) in id_classes.iter_mut().zip(sequence.id_classes) {
                *total += count;
            }
            allows += sequence
                .client
                .calls
                .lock()
                .expect("calls")
                .iter()
                .filter(|call| call[1]["behavior"] == "allow")
                .count();
        }
    }
    assert!(id_classes.iter().all(|count| *count > 0), "{id_classes:?}");
    assert!(
        allows > 100,
        "exercise backend allow calls, including failed writes"
    );
    println!(
        "{} sequences, {} steps, id classes {:?}, {} allow calls",
        per_seed * SEEDS.len(),
        per_seed * SEEDS.len() * steps,
        id_classes,
        allows
    );
    assert!(coverage.iter().all(|count| *count > 0), "{coverage:?}");
}

impl Sequence {
    fn new(seed: u64) -> Self {
        Self {
            app: AppState::new("thread".into()),
            client: Peer::default(),
            known: Vec::new(),
            facts: HashMap::new(),
            retired: HashSet::new(),
            accepts: HashMap::new(),
            previous: HashSet::new(),
            random: seed.max(1),
            id_classes: [0; 5],
        }
    }

    fn next(&mut self) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }

    fn arbitrary_id(&self, random: u64) -> EntryId {
        match random % 5 {
            0 if !self.known.is_empty() => self.known[(random / 5) as usize % self.known.len()],
            1 => self
                .active_id()
                .unwrap_or_else(|| EntryId::for_approval_test(Uuid::nil(), random)),
            2 => self
                .app
                .session
                .approval()
                .queued(ApprovalQueue::Claude)
                .front()
                .map(|p| p.entry_id())
                .unwrap_or_else(|| EntryId::for_approval_test(Uuid::nil(), random)),
            3 if !self.known.is_empty() => self.known[0].unallocated_for_approval_test(),
            _ => EntryId::for_approval_test(Uuid::from_u128(0xfeed), random),
        }
    }

    fn apply(&mut self, operation: u64, random: u64) {
        let id = self.arbitrary_id(random >> 8);
        self.observe_id(id);
        let queue = if random & 128 == 0 {
            ApprovalQueue::Claude
        } else {
            ApprovalQueue::Codex
        };
        match operation {
            0..=2 => self.app.set_pending_approval(draft(random)),
            3 | 4 => {
                self.app.session.enqueue_approval(draft(random), queue);
            }
            5 | 6 => self.app.session.focus_approval(id, queue),
            7 => {
                let retired = self.app.session.promote_approval(queue);
                self.app.drop_retired_approval_hints(retired);
            }
            8 => {
                let retired = self.app.session.retire_approval(id);
                self.app.drop_retired_approval_hints(retired);
            }
            9 => {
                let retired = self.app.session.clear_approval_queue(queue);
                self.app.drop_retired_approval_hints(retired);
            }
            10..=12 => self.accept(),
            13 => self.answer_target(id),
            14 => self.app.clear_pending_approval(),
            15 => self.duplicate(false),
            16 => self.duplicate(true),
            17 => self.app.reset_for_forked_thread("fork"),
            18 => self.app.reset_preserving_approvals("native reset"),
            19 | 20 => self.replace_history(operation == 20),
            21 => self.app.session.set_thread_id("switch".into()),
            22 => self.decline(),
            23 => {
                self.app.session.reset_for_forked_thread();
            }
            24 => respond_to_pending_approval(
                &self.client,
                &mut self.app,
                ApprovalChoice::AcceptForSession,
            ),
            25 => {
                self.client.fail.set(!self.client.fail.get());
                self.accept();
            }
            26 => assert_eq!(
                self.app.session.begin_approval_answer(id),
                self.active_id() == Some(id)
            ),
            27 => self.check_submitted(id),
            _ => unreachable!(),
        };
    }

    fn observe_id(&mut self, id: EntryId) {
        let class = if self.active_id() == Some(id) {
            0
        } else if self
            .app
            .session
            .approval()
            .live()
            .any(|p| p.entry_id() == id)
        {
            1
        } else if self.retired.contains(&id) {
            2
        } else if self
            .known
            .first()
            .is_some_and(|p| p.unallocated_for_approval_test() == id)
        {
            4
        } else {
            3
        };
        self.id_classes[class] += 1;
    }

    fn active_id(&self) -> Option<EntryId> {
        self.app.session.approval().pending().map(|p| p.entry_id())
    }

    fn replace_history(&mut self, new: bool) {
        let mode = if new {
            HistoryMode::NewConversation
        } else {
            HistoryMode::SameThread
        };
        self.app.session.replace_history(Vec::new(), mode);
    }

    fn check_submitted(&self, id: EntryId) {
        if let Some(answer) = self.app.session.submitted_approval_answer(id) {
            assert!(!self.retired.contains(&id));
            assert_eq!(answer, &self.facts[&id]);
        }
    }

    fn duplicate(&mut self, change: bool) {
        let Some(pending) = self.app.session.approval().pending() else {
            return;
        };
        let mut draft = pending.to_draft();
        if change {
            draft.requested_permissions.as_mut().expect("payload")["command"] = json!(self.random);
        }
        self.app.set_pending_approval(draft);
    }

    fn accept(&mut self) {
        let active = self.active_id();
        if let Some(id) = active {
            *self.accepts.entry(id).or_default() += 1;
        }
        let before = self.client.calls.lock().expect("calls").len();
        respond_to_pending_approval(&self.client, &mut self.app, ApprovalChoice::Accept);
        let calls = self.client.calls.lock().expect("calls");
        if self.client.fail.get() {
            assert_eq!(self.active_id(), active);
        }
        if calls.len() > before {
            assert_eq!(calls.len(), before + 1);
            let id = active.expect("allow must answer an active card");
            assert!(
                self.accepts[&id] >= 2,
                "allow must follow two accepts on this id"
            );
            assert_eq!(calls[before][0], self.facts[&id].request_id);
        }
    }

    fn decline(&mut self) {
        let choice = if self.random & 64 == 0 {
            ApprovalChoice::Decline
        } else {
            ApprovalChoice::Cancel
        };
        respond_to_pending_approval(&self.client, &mut self.app, choice);
    }

    fn answer_target(&self, id: EntryId) {
        let active = self.active_id();
        let answer = self.app.session.approval_answer(id);
        assert_eq!(answer.is_some(), active == Some(id));
        if let Some(answer) = answer {
            assert_eq!(answer, self.facts[&id]);
        }
    }

    fn check(&mut self) {
        let live: HashSet<_> = self
            .app
            .session
            .approval()
            .live()
            .map(|p| p.entry_id())
            .collect();
        let count = self
            .app
            .session
            .approval()
            .queued(ApprovalQueue::Claude)
            .len()
            + self
                .app
                .session
                .approval()
                .queued(ApprovalQueue::Codex)
                .len()
            + usize::from(self.app.session.approval().pending().is_some());
        assert_eq!(live.len(), count, "active and queued ids must be distinct");
        self.retired.extend(self.previous.difference(&live));
        assert!(
            live.is_disjoint(&self.retired),
            "a retired id cannot reappear"
        );
        for card in self.app.session.approval().live() {
            let id = card.entry_id();
            if let Some(facts) = self.facts.get(&id) {
                assert_eq!(facts, card.facts(), "one id always names the same facts");
            } else {
                self.facts.insert(id, card.to_draft());
                self.known.push(id);
            }
        }
        for id in &self.known {
            self.answer_target(*id);
            self.check_submitted(*id);
            if self.retired.contains(id) {
                assert!(self.app.session.submitted_approval_answer(*id).is_none());
            }
        }
        self.app.assert_approval_hint_invariants();
        self.previous = live;
    }
}

fn draft(random: u64) -> ApprovalRequestDraft {
    ApprovalRequestDraft {
        request_id: json!(random % 4),
        method: format!("native/{}", random % 3),
        kind: [
            ApprovalRequestKind::CommandExecution,
            ApprovalRequestKind::FileChange,
            ApprovalRequestKind::Permissions,
            ApprovalRequestKind::LegacyExecCommand,
            ApprovalRequestKind::LegacyApplyPatch,
            ApprovalRequestKind::ClaudeExitPlanMode,
            ApprovalRequestKind::ClaudeNative,
            ApprovalRequestKind::ClaudeQuestion,
            ApprovalRequestKind::CodexQuestion,
            ApprovalRequestKind::CodexForm,
        ][(random / 32) as usize % 10],
        title: format!("Allow {}?", random % 7),
        detail_lines: vec![format!("command {}", random % 11)],
        requested_permissions: Some(json!({"defaultToNo":true,"command":random % 17})),
        can_accept_for_session: random & 8 == 0,
        can_decline: random & 16 == 0,
        can_cancel: random & 32 == 0,
    }
}

#[derive(Default)]
struct Peer {
    calls: Mutex<Vec<Value>>,
    fail: Cell<bool>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, _: &str, _: Value, _: Duration) -> anyhow::Result<String> {
        anyhow::bail!("no request expected")
    }

    fn respond(&self, id: &Value, answer: Value) -> anyhow::Result<()> {
        self.calls.lock().expect("calls").push(json!([id, answer]));
        if self.fail.get() {
            anyhow::bail!("fixed answer failure");
        }
        Ok(())
    }

    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("no error response expected")
    }

    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused")
    }

    fn stop(&mut self) {}
}
