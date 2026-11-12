//! Runs interrupt schedules through real helper pipes and the native reader thread.

use std::process::Command;

use serde_json::{json, Value};

use super::event_race_support::DEADLINE;
use super::event_test_support::{command_client, records, submit};
use crate::backend::BackendClient;

#[test]
fn result_rejected_reply_idle_uses_native_success() {
    check_schedule("result,reply,idle", false, false);
}

#[test]
fn idle_result_rejected_reply_waits_for_the_reply() {
    check_schedule("idle,result,reply", false, false);
}

#[test]
fn result_accepted_reply_idle_uses_confirmed_interrupt() {
    check_schedule("result,reply,idle", true, false);
}

#[test]
fn rejected_reply_result_idle_uses_native_success() {
    check_schedule("reply,result,idle", false, false);
}

#[test]
fn two_prompts_interrupt_only_the_captured_first_prompt() {
    check_schedule("idle,result,reply", true, true);
}

#[test]
fn eof_rejects_unanswered_interrupt_after_already_queued_result_and_idle() {
    let mut command = Command::new("python3");
    command.args(["-c", PEER, "result,idle,exit", "no"]);
    let (client, receiver) = command_client(command, 1024);
    let id = submit(&client);
    records(&client, &receiver);
    client
        .call("turn/interrupt", json!({}), DEADLINE)
        .expect_err("helper exits without reply");
    let actual = super::event_race_support::through(&client, &receiver, "claude/error");
    let completed: Vec<Value> = actual
        .iter()
        .filter(|line| line["method"] == "turn/completed")
        .map(|line| line["params"]["turn"].clone())
        .collect();
    assert_eq!(completed, [json!({"id":id,"status":"completed"})]);
}

#[test]
fn fatal_fact_after_result_and_idle_waits_for_the_actual_interrupt_reply() {
    let mut command = Command::new("python3");
    command.args(["-c", PEER, "result,idle,error,reply", "yes"]);
    let (client, receiver) = command_client(command, 1024);
    let id = submit(&client);
    records(&client, &receiver);
    client
        .call("turn/interrupt", json!({}), DEADLINE)
        .expect("interrupt accepted");
    let actual = super::event_race_support::through(&client, &receiver, "claude/error");
    let completed: Vec<Value> = actual
        .iter()
        .filter(|line| line["method"] == "turn/completed")
        .map(|line| line["params"]["turn"].clone())
        .collect();
    assert_eq!(completed, [json!({"id":id,"status":"interrupted"})]);
}

fn check_schedule(order: &str, accepted: bool, two: bool) {
    let mut command = Command::new("python3");
    command.args(["-c", PEER, order, if accepted { "yes" } else { "no" }]);
    let (client, receiver) = command_client(command, 1024);
    let first = submit(&client);
    let second = two.then(|| submit(&client));
    records(&client, &receiver);
    let result = client.call("turn/interrupt", json!({}), DEADLINE);
    assert_eq!(result.is_ok(), accepted);
    client
        .helper_call("barrier", json!({}), DEADLINE)
        .expect("reader consumed all frames");
    let completed: Vec<Value> = records(&client, &receiver)
        .into_iter()
        .filter(|line| line["method"] == "turn/completed")
        .map(|line| line["params"]["turn"].clone())
        .collect();
    let mut expected =
        vec![json!({"id":first,"status":if accepted {"interrupted"} else {"completed"}})];
    if let Some(second) = second {
        expected.push(json!({"id":second,"status":"completed"}));
    }
    assert_eq!(completed, expected);
}

const PEER: &str = r#"
import sys,json
order=sys.argv[1].split(','); accepted=sys.argv[2]=='yes'; prompts=[]
def emit(frame): print(json.dumps(frame),flush=True)
def sdk(message): emit({'v':1,'type':'event','event':'sdk','message':message})
def result(uuid): sdk({'type':'result','subtype':'success','is_error':False,'user_message_uuid':uuid})
def idle(): sdk({'type':'system','subtype':'session_state_changed','state':'idle'})
def bind(uuid): sdk({'type':'assistant','user_message_uuid':uuid,'message':{'role':'assistant','content':[]}})
for line in sys.stdin:
 r=json.loads(line); method=r['method']
 if method=='prompt':
  prompts.append(r['params']['uuid'])
  if len(prompts)==1: bind(prompts[0])
 if method=='interrupt':
  for event in order:
   if event=='oversized': emit({'v':1,'type':'event','event':'error','error':{'message':'x'*(9*1024*1024)}})
   if event=='error': emit({'v':1,'type':'event','event':'error','error':{'message':'terminal error after native completion'}})
   if event=='exit': sys.exit(0)
   if event=='result': result(prompts[0])
   if event=='idle': idle()
   if event=='reply': emit({'v':1,'type':'reply','id':r['id'],'ok':accepted,'result':{},'error':{'message':'rejected interrupt'}})
  if len(prompts)==2:
   bind(prompts[1]); result(prompts[1]); idle()
  continue
 emit({'v':1,'type':'reply','id':r['id'],'ok':True,'result':{}})
 if method=='shutdown': break
"#;
