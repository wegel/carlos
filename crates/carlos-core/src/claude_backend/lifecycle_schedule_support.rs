//! Replays separate reader, forwarder, and handler cursors against main and the backend.

use std::collections::VecDeque;

use serde_json::{json, Value};

use super::event_handler::EventHandler;
use super::lifecycle_reference_dispatch::ReferenceDriver;

#[derive(Clone, Copy, Debug)]
pub(super) struct Interrupt {
    pub(super) handled: usize,
    pub(super) read: usize,
    pub(super) reply: usize,
    pub(super) accepted: Option<bool>,
}

#[derive(Default)]
pub(super) struct Counts {
    pub(super) schedules: usize,
    pub(super) depth: usize,
}

struct Replay<'a> {
    facts: &'a [String],
    read: usize,
    handled: usize,
    pipe: VecDeque<&'a str>,
    forwarded: VecDeque<&'a str>,
    owner: EventHandler,
    reference: ReferenceDriver,
    actual: Vec<String>,
    expected: Vec<String>,
    depth: usize,
}

impl Replay<'_> {
    fn read_to(&mut self, end: usize) {
        while self.read < end {
            self.pipe.push_back(&self.facts[self.read]);
            self.read += 1;
            self.depth += 1;
        }
    }

    fn forward(&mut self) {
        self.depth += self.pipe.len();
        self.forwarded.append(&mut self.pipe);
    }

    fn handle_to(&mut self, end: usize) {
        self.read_to(end.max(self.read));
        self.forward();
        while self.handled < end {
            let line = self.forwarded.pop_front().expect("forwarded fact");
            self.actual.extend(self.owner.handle(line));
            self.expected.extend(self.reference.handle(line));
            self.handled += 1;
            self.depth += 1;
        }
    }

    fn interrupt(&mut self, call: Interrupt, forward_before_call: bool) {
        self.handle_to(call.handled);
        self.read_to(call.read.max(self.read));
        if forward_before_call {
            self.forward();
        }
        // main:turn_interrupt::interrupt_active_turn blocks this handler until the call returns.
        let captured = self.owner.lifecycle.begin_interrupt();
        let original = self.reference.state.begin_interrupt();
        assert_eq!(captured, original);
        self.depth += 1;
        self.read_to(call.reply.max(self.read));
        self.forward();
        self.owner
            .lifecycle
            .finish_interrupt(&captured, call.accepted == Some(true));
        if call.accepted != Some(true) {
            self.reference.state.rollback_interrupt(&original);
        }
        self.depth += 1;
        if call.accepted.is_none() {
            // main's bounded timeout stops the helper; EOF follows already queued native facts.
            self.read_to(self.facts.len());
            self.handle_to(self.facts.len());
            let eof = fact("claude/error", json!({"error":{"message":"timeout EOF"}}));
            self.actual.extend(self.owner.handle(&eof));
            self.expected.extend(self.reference.handle(&eof));
            self.depth += 3;
        }
    }
}

pub(super) fn check(facts: &[String], calls: &[Interrupt], forwarded: bool, counts: &mut Counts) {
    let mut replay = Replay {
        facts,
        read: 0,
        handled: 0,
        pipe: VecDeque::new(),
        forwarded: VecDeque::new(),
        owner: EventHandler::default(),
        reference: ReferenceDriver::new(),
        actual: Vec::new(),
        expected: Vec::new(),
        depth: 0,
    };
    for call in calls {
        replay.interrupt(*call, forwarded);
    }
    replay.handle_to(facts.len());
    assert_eq!(
        replay.actual, replay.expected,
        "facts={facts:?}, calls={calls:?}"
    );
    counts.schedules += 1;
    counts.depth = counts.depth.max(replay.depth);
}

pub(super) fn fact(method: &str, params: Value) -> String {
    json!({"method":method,"params":params}).to_string()
}

pub(super) fn base(prompts: usize, mut variant: usize) -> Vec<String> {
    let mut facts = Vec::new();
    for id in 0..prompts {
        facts.push(fact(
            "claude/promptSubmitted",
            json!({"uuid":id.to_string()}),
        ));
    }
    for id in 0..prompts {
        let status = variant % 3;
        let idle_first = variant % 6 >= 3;
        variant /= 6;
        facts.push(fact(
            "claude/nativeBind",
            json!({"user_message_uuid":id.to_string()}),
        ));
        let result = fact(
            "claude/nativeResult",
            json!({
                "user_message_uuid":id.to_string(), "subtype":"success",
                "is_error":status == 1, "terminal_reason":if status == 2 {"aborted_tools"} else {""},
                "error":if status == 1 {json!("failed")} else {Value::Null}
            }),
        );
        let idle = fact("claude/activity", json!({"state":"idle"}));
        facts.extend(if idle_first {
            [idle, result]
        } else {
            [result, idle]
        });
    }
    facts
}

pub(super) fn disruptions() -> Vec<Vec<String>> {
    vec![
        vec![fact(
            "claude/conversationReset",
            json!({"new_conversation_id":"reset"}),
        )],
        vec![fact(
            "claude/conversationReset",
            json!({"new_conversation_id":"clear","user_message_uuid":"0"}),
        )],
        vec![fact("claude/nativeBind", json!({"user_message_uuid":"1"}))],
        vec![fact(
            "claude/error",
            json!({"error":{"message":"helper EOF"}}),
        )],
        vec![fact(
            "claude/error",
            json!({"error":{"message":"stderr failure"}}),
        )],
        vec![fact("claude/closed", json!({"reason":"closed"}))],
        vec![
            fact(
                "claude/sessionChanged",
                json!({"generation":2,"sessionId":"new"}),
            ),
            fact(
                "claude/nativeResult",
                json!({"generation":1,"user_message_uuid":"0","subtype":"success"}),
            ),
        ],
        vec![
            fact(
                "claude/conversationReset",
                json!({"epoch":2,"new_conversation_id":"reset"}),
            ),
            fact(
                "claude/nativeResult",
                json!({"epoch":1,"user_message_uuid":"0","subtype":"success"}),
            ),
        ],
    ]
}
