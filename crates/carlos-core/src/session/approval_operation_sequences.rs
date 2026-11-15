//! Preserve native card identity and claim invariants over the original fixed-seed corpus.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::json;
use uuid::Uuid;

use super::approval_control_test_support::Peer;
use super::approval_state::{ApprovalQueue, ApprovalRequestDraft, ApprovalRequestKind};
use super::entry_id::EntryId;
use super::history_identity::HistoryMode;
use super::state::SessionState;
use crate::backend::BackendKind;

const SEEDS: [u64; 4] = [1, 0xcafe, 0xdeadbeef, 0x9e3779b97f4a7c15];
const OPERATIONS: u64 = 28;

struct Sequence {
    core: SessionState,
    client: Peer,
    known: Vec<EntryId>,
    facts: HashMap<EntryId, ApprovalRequestDraft>,
    retired: HashSet<EntryId>,
    previous: HashSet<EntryId>,
    random: u64,
    id_classes: [usize; 5],
}

#[test]
fn twenty_thousand_public_approval_sequences_preserve_identity_and_claims() {
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
                .filter(|call| call.1["behavior"] == "allow")
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
            core: SessionState::new("thread".into(), Default::default(), false),
            client: Peer::new(BackendKind::Claude),
            known: Vec::new(),
            facts: HashMap::new(),
            retired: HashSet::new(),
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
                .core
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
            0..=2 => {
                let _ = self.core.set_pending_approval(draft(random));
            }
            3 | 4 => {
                self.core.enqueue_approval(draft(random), queue);
            }
            5 | 6 => self.core.focus_approval(id, queue),
            7 => {
                let _ = self.core.promote_approval(queue);
            }
            8 => {
                let _ = self.core.retire_approval(id);
            }
            9 => {
                let _ = self.core.clear_approval_queue(queue);
            }
            10..=12 => self.accept(),
            13 => self.answer_target(id),
            14 => {
                self.core.clear_pending_session_approval();
            }
            15 => self.duplicate(false),
            16 => self.duplicate(true),
            17 => self.reset(false),
            18 => self.reset(true),
            19 | 20 => self.replace_history(operation == 20),
            21 => self.core.set_thread_id("switch".into()),
            22 => self.decline(),
            23 => {
                self.core.reset_for_forked_thread();
            }
            24 => self.respond(Decision::Session),
            25 => {
                self.client.fail.fetch_xor(true, Ordering::SeqCst);
                self.accept();
            }
            26 => assert_eq!(
                self.core.begin_approval_answer(id),
                self.active_id() == Some(id)
            ),
            27 => self.check_submitted(id),
            _ => unreachable!(),
        };
    }

    fn observe_id(&mut self, id: EntryId) {
        let class = if self.active_id() == Some(id) {
            0
        } else if self.core.approval().live().any(|p| p.entry_id() == id) {
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
        self.core.approval().pending().map(|p| p.entry_id())
    }

    fn replace_history(&mut self, new: bool) {
        let mode = if new {
            HistoryMode::NewConversation
        } else {
            HistoryMode::SameThread
        };
        self.core.replace_history(Vec::new(), mode);
    }

    fn check_submitted(&self, id: EntryId) {
        if let Some(answer) = self.core.submitted_approval_answer(id) {
            assert!(!self.retired.contains(&id));
            assert_eq!(answer, &self.facts[&id]);
        }
    }

    fn duplicate(&mut self, change: bool) {
        let Some(pending) = self.core.approval().pending() else {
            return;
        };
        let mut draft = pending.to_draft();
        if change {
            draft.requested_permissions.as_mut().expect("payload")["command"] = json!(self.random);
        }
        let _ = self.core.set_pending_approval(draft);
    }

    fn accept(&mut self) {
        self.respond(Decision::Once);
    }

    fn decline(&mut self) {
        self.respond(if self.random & 64 == 0 {
            Decision::Deny
        } else {
            Decision::Cancel
        });
    }

    fn respond(&mut self, decision: Decision) {
        let active = self.active_id();
        let before = self.client.calls.lock().expect("calls").len();
        if let Some(id) = active {
            if let Ok(work) = self.core.prepare_engine_answer(
                &self.client,
                id,
                ApprovalAnswer::Decision { decision },
            ) {
                let _ = self
                    .core
                    .finish_engine_control(&self.client, work.execute(&self.client.commands));
            }
        }
        let calls = self.client.calls.lock().expect("calls");
        if self.client.fail.load(Ordering::SeqCst) {
            assert_eq!(self.active_id(), active);
        }
        if calls.len() > before {
            assert_eq!(calls.len(), before + 1);
            let id = active.expect("a write must answer an exact live card");
            assert_eq!(calls[before].0, self.facts[&id].request_id);
        }
    }

    fn reset(&mut self, preserve: bool) {
        self.core
            .adopt_native_thread(if preserve { "native reset" } else { "fork" }.into());
        self.core.reset_for_forked_thread();
        self.core.reset_prompt_history();
        if !preserve {
            self.core.clear_pending_session_approval();
        }
    }

    fn answer_target(&self, id: EntryId) {
        let active = self.active_id();
        let answer = self.core.approval_answer(id);
        assert_eq!(answer.is_some(), active == Some(id));
        if let Some(answer) = answer {
            assert_eq!(answer, self.facts[&id]);
        }
    }

    fn check(&mut self) {
        let live: HashSet<_> = self.core.approval().live().map(|p| p.entry_id()).collect();
        let count = self.core.approval().queued(ApprovalQueue::Claude).len()
            + self.core.approval().queued(ApprovalQueue::Codex).len()
            + usize::from(self.core.approval().pending().is_some());
        assert_eq!(live.len(), count, "active and queued ids must be distinct");
        self.retired.extend(self.previous.difference(&live));
        assert!(
            live.is_disjoint(&self.retired),
            "a retired id cannot reappear"
        );
        for card in self.core.approval().live() {
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
                assert!(self.core.submitted_approval_answer(*id).is_none());
            }
        }
        self.core.messages().assert_invariants();
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
