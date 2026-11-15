//! Drive reply/result/idle/EOF orders through the real owner and actual helper pipes.

use carlos_proto::commands::SendPolicy;
use carlos_proto::session::Availability;
use carlos_proto::submissions::OperationState;
use serde_json::{json, Value};

use super::interrupt_owner_support::{operation, Fixture, MARKER, TIMEOUT};
use crate::engine::EngineCommand;

#[test]
fn actual_owner_waits_for_accepted_rejected_or_eof_interrupt_before_success_completion() {
    for reply in ["accepted", "rejected", "eof"] {
        check("success", reply, false);
    }
}

#[test]
fn actual_owner_preserves_native_abort_and_error_before_accepted_ack() {
    for native in ["failed", "aborted"] {
        check(native, "accepted", false);
    }
}

#[test]
fn actual_owner_completes_before_the_first_bounded_terminal_and_rejects_new_writes() {
    for reply in ["accepted", "rejected", "eof"] {
        check("success", reply, true);
    }
}

fn check(native: &str, reply: &str, fatal: bool) {
    let fixture = Fixture::start(PEER, native, fatal);
    fixture.request(EngineCommand::Send {
        operation: operation(1),
        text: "held".into(),
        policy: SendPolicy::Queue,
    });
    fixture.outcome(1);
    let started = fixture.until(|snapshot| {
        snapshot.info.current_turn.is_some()
            && snapshot
                .entries
                .iter()
                .any(|entry| entry.text.contains("Answer: held"))
    });
    let turn = started.info.current_turn.expect("current native turn");
    fixture.request(EngineCommand::Interrupt {
        operation: operation(2),
        turn,
    });
    let at_step = fixture.until(|snapshot| {
        snapshot
            .entries
            .iter()
            .any(|entry| entry.text.contains(MARKER))
    });
    let before = completions(&fixture);
    if native == "success" {
        assert!(
            before.is_empty(),
            "successful result waits for its captured control: reply={reply}, facts={:?}",
            fixture.facts()
        );
        assert!(at_step.info.current_turn.is_some());
    } else {
        assert_eq!(
            before.len(),
            1,
            "native terminal error/abort does not wait for intent"
        );
    }
    if fatal {
        assert_closed_dispatch(&fixture);
    }
    finish_check(&fixture, native, reply, fatal);
}

fn assert_closed_dispatch(fixture: &Fixture) {
    assert!(
        fixture
            .commands
            .helper_call("interrupt", json!({}), TIMEOUT)
            .is_err(),
        "held closure rejects new writes at dispatch"
    );
    fixture.request(EngineCommand::Send {
        operation: operation(3),
        text: "not resent".into(),
        policy: SendPolicy::Queue,
    });
    assert!(matches!(
        fixture.outcome(3).state,
        OperationState::Rejected { .. }
    ));
}

fn finish_check(fixture: &Fixture, native: &str, reply: &str, fatal: bool) {
    fixture
        .commands
        .helper_call("release", json!({"reply":reply}), TIMEOUT)
        .expect("release exact held reply");
    let outcome = fixture.outcome(2);
    assert_eq!(
        matches!(outcome.state, OperationState::Accepted { .. }),
        reply == "accepted"
    );
    let ended = fixture.until(|snapshot| {
        snapshot.info.current_turn.is_none()
            && (!(fatal || reply == "eof") || snapshot.info.availability == Availability::Closed)
    });
    let expected = match (native, reply) {
        ("failed", _) => "failed",
        ("aborted", _) | (_, "accepted") => "interrupted",
        _ => "completed",
    };
    let completed = completions(fixture);
    assert_eq!(
        completed.len(),
        1,
        "one completion for the exact accepted prompt"
    );
    assert_eq!(completed[0]["params"]["turn"]["status"], expected);
    assert_eq!(
        ended
            .entries
            .iter()
            .filter(|entry| entry.text == "Turn interrupted")
            .count(),
        usize::from(expected == "interrupted")
    );
    if fatal || reply == "eof" {
        assert_completion_before_error(fixture, fatal);
    }
}

fn completions(fixture: &Fixture) -> Vec<Value> {
    fixture
        .facts()
        .into_iter()
        .filter(|fact| fact["method"] == "turn/completed")
        .collect()
}

fn assert_completion_before_error(fixture: &Fixture, fatal: bool) {
    let facts = fixture.facts();
    let completed = facts
        .iter()
        .position(|fact| fact["method"] == "turn/completed")
        .expect("completion");
    let terminal = facts
        .iter()
        .position(|fact| fact["method"] == "claude/error")
        .expect("terminal cause");
    assert!(
        completed < terminal,
        "confirmed result remains before closure"
    );
    let message = facts[terminal]["params"]["error"]["message"]
        .as_str()
        .expect("bounded first cause");
    assert!(message.len() <= 512 + " [details truncated]".len());
    if fatal {
        assert!(message.starts_with("first terminal cause"));
    }
}

const PEER: &str = r#"
import json,sys
native=sys.argv[1]; fatal=sys.argv[2]=='yes'; held=None; prompt=None
def emit(value): print(json.dumps(value),flush=True)
def sdk(value): emit({'v':1,'type':'event','event':'sdk','message':value})
def answer(request,ok=True): emit({'v':1,'type':'reply','id':request['id'],'ok':ok,'result':{},'error':{'message':'native interrupt refused'}})
for line in sys.stdin:
 r=json.loads(line); method=r['method']
 if method=='prompt':
  prompt=r['params']['uuid']; answer(r)
  sdk({'type':'assistant','user_message_uuid':prompt,'message':{'role':'assistant','content':[{'type':'text','text':'Answer: held'}]}})
  continue
 if method=='interrupt':
  held=r
  sdk({'type':'result','subtype':'success' if native=='success' else 'error_during_execution','is_error':native!='success','terminal_reason':'aborted_tools' if native=='aborted' else '', 'errors':['native failure'] if native=='failed' else [],'user_message_uuid':prompt})
  sdk({'type':'system','subtype':'session_state_changed','state':'idle'})
  if fatal: emit({'v':1,'type':'event','event':'error','error':{'message':'first terminal cause '+('x'*10000)}})
  emit({'v':1,'type':'event','event':'stderr','text':'owner has applied result and idle'})
  continue
 if method=='release':
  reply=r['params']['reply']
  if reply!='eof': answer(held,reply=='accepted')
  answer(r)
  if reply=='eof': sys.exit(0)
  continue
 answer(r)
 if method=='shutdown': break
"#;
