//! Exercise real OS pipes, native request IDs and budgets without model calls.

use std::io::Cursor;
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol::AppServerClient;
use crate::protocol_events::EventQueue;
use crate::protocol_params::initialize_client;
use crate::protocol_reader::read_line;

fn peer(script: &str) -> AppServerClient {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", script]);
    AppServerClient::start_command(command).expect("isolated pipe peer")
}

#[test]
fn initialize_notice_precedes_any_later_request() {
    let client = peer("import sys,json\na=json.loads(input()); print(json.dumps({'id':a['id'],'result':{}}))\nb=json.loads(input()); c=json.loads(input()); print(json.dumps({'id':c['id'],'result':{'notice':b,'request':c}}))\nsys.stdin.read()");
    initialize_client(&client).expect("handshake");
    let reply = client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("model list");
    let reply: Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["result"]["notice"]["method"], "initialized");
    assert_eq!(reply["result"]["request"]["method"], "model/list");
}

#[test]
fn eof_settles_waiting_call_and_rejects_later_work() {
    let mut client = peer("input()");
    let events = client.take_events_rx().expect("events");
    let started = Instant::now();
    let error = client
        .call("turn/start", json!({}), Duration::from_secs(10))
        .expect_err("EOF");
    assert!(error.to_string().contains("stdout closed"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(2));
    let notice = events
        .recv_timeout(Duration::from_secs(1))
        .expect("closure notice");
    assert!(notice.contains("carlos/codex/closed"));
    assert!(client
        .call("turn/start", json!({}), Duration::from_secs(10))
        .is_err());
}

#[test]
fn blocked_stdin_uses_the_call_deadline_and_stop_does_not_wait_for_the_writer() {
    let client = peer("import time; time.sleep(60)");
    let started = Instant::now();
    let error = client
        .call(
            "turn/start",
            json!({"text":"x".repeat(2 * 1024 * 1024)}),
            Duration::from_millis(150),
        )
        .expect_err("blocked write");
    assert!(error.to_string().contains("stdin deadline"), "{error}");
    client.stop();
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_healthy_large_write_can_take_longer_than_half_a_second() {
    let client = peer("import time,json,sys\ntime.sleep(.65)\na=json.loads(input()); print(json.dumps({'id':a['id'],'result':len(a['params']['text'])})); sys.stdin.read()");
    let length = 7 * 1024 * 1024;
    let result = client
        .call(
            "turn/start",
            json!({"text":"x".repeat(length)}),
            Duration::from_secs(4),
        )
        .expect("healthy large frame");
    assert_eq!(
        serde_json::from_str::<Value>(&result).expect("JSON")["result"],
        length
    );
}

#[test]
fn a_slow_read_does_not_kill_native_work_or_leak_its_late_reply() {
    let mut client = peer("import json,time,sys\nfor line in sys.stdin:\n a=json.loads(line)\n if a['method']=='model/list': time.sleep(.2)\n print(json.dumps({'id':a['id'],'result':{'ok':True}}))");
    let events = client.take_events_rx().expect("events");
    assert!(client
        .call("model/list", json!({}), Duration::from_millis(30))
        .is_err());
    client
        .call("turn/start", json!({}), Duration::from_secs(2))
        .expect("still healthy");
    assert!(
        events.try_recv().is_err(),
        "retired read reply must be discarded"
    );
}

#[test]
fn an_uncertain_mutation_timeout_stops_native_work() {
    let client = peer("import time; input(); time.sleep(60)");
    assert!(client
        .call("turn/start", json!({}), Duration::from_millis(50))
        .is_err());
    assert!(client
        .call("model/list", json!({}), Duration::from_secs(1))
        .is_err());
}

#[test]
fn callback_ids_and_unknown_additive_messages_reach_the_host_unchanged() {
    let mut client = peer("import json,sys\nfor id in ['9',9,2.5]:\n print(json.dumps({'id':id,'method':'future/question','params':{'extra':'keep'}}))\n answer=json.loads(input()); print(json.dumps({'method':'future/answer','params':answer}))\nsys.stdin.read()");
    let events = client.take_events_rx().expect("events");
    for id in [json!("9"), json!(9), json!(2.5)] {
        let question = events
            .recv_timeout(Duration::from_secs(2))
            .expect("callback");
        let question: Value = serde_json::from_str(&question).expect("JSON");
        assert_eq!(question["id"], id);
        client
            .respond(&id, json!({"ok":true}))
            .expect("callback reply");
        let answer = events
            .recv_timeout(Duration::from_secs(2))
            .expect("answer echo");
        let answer: Value = serde_json::from_str(&answer).expect("JSON");
        assert_eq!(answer["params"]["id"], id);
    }
}

#[test]
fn async_reply_retains_request_context_and_raw_native_extra_fields() {
    let mut client = peer("import json,sys\nfor line in sys.stdin:\n a=json.loads(line); print(json.dumps({'id':a['id'],'result':{'future':42}}))");
    let events = client.take_events_rx().expect("events");
    let params = json!({"threadId":"child","processId":"proc"});
    client
        .request_native_control("thread/backgroundTerminals/list", params.clone())
        .expect("async read");
    let line = events
        .recv_timeout(Duration::from_secs(2))
        .expect("reply event");
    let event: Value = serde_json::from_str(&line).expect("JSON");
    assert_eq!(
        event["params"]["requestMethod"],
        "thread/backgroundTerminals/list"
    );
    assert_eq!(event["params"]["requestParams"], params);
    assert_eq!(event["params"]["response"]["result"]["future"], 42);
}

#[test]
fn near_limit_queued_input_keeps_the_full_native_reply_without_echoing_input_twice() {
    let mut client = peer("import json,sys\nfor line in sys.stdin:\n a=json.loads(line);p=a['params'];print(json.dumps({'id':a['id'],'result':{'queuedSubmission':{'id':'native-id','input':p['input'],'clientUserMessageId':p['clientUserMessageId']},'localContextLeaked':'epoch' in p}}))");
    let events = client.take_events_rx().expect("events");
    let length = 8 * 1024 * 1024 - 16 * 1024;
    let params = json!({"threadId":"root","clientUserMessageId":"human-id","input":[{"type":"text","text":"x".repeat(length)}]});
    client
        .request_control_with_context("thread/queue/add", params, json!({"epoch":2,"token":7}))
        .expect("prompt-sized queue control");
    let line = events
        .recv_timeout(Duration::from_secs(5))
        .expect("full queue response");
    assert!(
        line.len() < 9 * 1024 * 1024,
        "input is retained only in the native reply"
    );
    let event: Value = serde_json::from_str(&line).expect("JSON event");
    assert!(event["params"]["requestParams"].get("input").is_none());
    assert_eq!(
        event["params"]["response"]["result"]["queuedSubmission"]["input"][0]["text"]
            .as_str()
            .expect("full text")
            .len(),
        length
    );
    assert_eq!(
        event["params"]["requestContext"],
        json!({"epoch":2,"token":7})
    );
    assert_eq!(
        event["params"]["response"]["result"]["localContextLeaked"],
        false
    );
    client.acknowledge_event(line.len());
}

#[test]
fn asynchronous_queue_write_does_not_hold_the_ui_while_native_stdin_is_busy() {
    let mut client = peer("import time,json,sys\ntime.sleep(.65)\na=json.loads(input());print(json.dumps({'id':a['id'],'result':{}}));sys.stdin.read()");
    let events = client.take_events_rx().expect("events");
    let params = json!({"threadId":"root","clientUserMessageId":"human-id","input":[{"type":"text","text":"x".repeat(1024*1024)}]});
    let started = Instant::now();
    client
        .request_control_with_context("thread/queue/add", params, json!({"token":1}))
        .expect("bounded async enqueue");
    assert!(
        started.elapsed() < Duration::from_millis(400),
        "UI waits for enqueue, not OS write"
    );
    events
        .recv_timeout(Duration::from_secs(3))
        .expect("healthy delayed native read");
}

#[test]
fn forwarding_frames_does_not_release_their_ui_budget() {
    let (queue, receiver) = EventQueue::new();
    for _ in 0..4096 {
        queue.send("{}".into()).expect("budgeted frame");
        receiver.recv().expect("forwarded to another queue");
    }
    assert!(queue.send("{}".into()).is_err());
    queue.terminal("fatal".into());
    assert_eq!(receiver.recv().expect("reserved failure"), "fatal");
    queue.handled(5); // The reserved diagnostic has its own real charge.
    queue.handled(2);
    queue
        .send("{}".into())
        .expect("UI acknowledgement frees slot");
}

#[test]
fn oversized_and_unterminated_frames_fail_before_unbounded_growth() {
    assert!(read_line(&mut Cursor::new(b"{\"missing\":true}")).is_err());
    assert!(read_line(&mut Cursor::new(vec![b'x'; 8 * 1024 * 1024 + 1])).is_err());
}

#[test]
fn a_native_event_flood_fails_pending_work_and_stops_the_process_without_a_consumer() {
    let mut client = peer("import json,sys,time\na=json.loads(input())\nfor n in range(10000): print(json.dumps({'method':'future/progress','params':{'n':n}}))\ntime.sleep(60)");
    let events = client.take_events_rx().expect("events");
    let error = client
        .call("turn/start", json!({}), Duration::from_secs(2))
        .expect_err("bounded event backlog");
    assert!(error.to_string().contains("4096 frames"), "{error}");
    let mut fatal = 0;
    while let Ok(event) = events.try_recv() {
        if event.contains("carlos/codex/closed") {
            fatal += 1;
        }
    }
    assert_eq!(fatal, 1, "reserved terminal diagnostic");
    client.stop();
    assert!(client.state.reap(), "native process ended");
}

#[test]
fn a_broken_stdin_pipe_settles_the_caller_and_reports_the_actual_write_error() {
    let mut client = peer("import os,json,time\nos.close(0);print(json.dumps({'method':'future/ready','params':{}}));time.sleep(60)");
    let events = client.take_events_rx().expect("events");
    events
        .recv_timeout(Duration::from_secs(2))
        .expect("stdin closed before request");
    let error = client
        .call("turn/start", json!({}), Duration::from_secs(2))
        .expect_err("broken stdin");
    assert!(error.to_string().contains("Write Codex stdin"), "{error}");
}

#[test]
fn oversized_outgoing_input_is_rejected_before_dispatch_without_killing_a_healthy_cli() {
    let client = peer("import json,sys\nfor line in sys.stdin:\n a=json.loads(line);print(json.dumps({'id':a['id'],'result':{}}))");
    let error = client
        .call(
            "turn/start",
            json!({"text":"x".repeat(8*1024*1024)}),
            Duration::from_secs(2),
        )
        .expect_err("oversized request");
    assert!(error.to_string().contains("exceeds 8 MiB"));
    client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("not dispatched, still healthy");
}

#[test]
fn async_pending_work_and_unhandled_event_bytes_are_bounded_independently() {
    let mut client = peer("import sys,json\nfor line in sys.stdin: print(json.dumps({'method':'fixture/read','params':{}}))");
    let events = client.take_events_rx().expect("native admission notices");
    for _ in 0..64 {
        client
            .request_native_control("future/slow", json!({}))
            .expect("pending slot");
        let line = events
            .recv_timeout(Duration::from_secs(2))
            .expect("native read without RPC reply");
        client.acknowledge_event(line.len());
    }
    assert!(client
        .request_native_control("future/slow", json!({}))
        .expect_err("pending cap")
        .to_string()
        .contains("64 pending"));
    let (queue, receiver) = EventQueue::new();
    for _ in 0..8 {
        queue
            .send("x".repeat(2 * 1024 * 1024))
            .expect("byte budget");
        receiver.recv().expect("forwarded");
    }
    assert!(
        queue.send("x".into()).is_err(),
        "forwarding must not release bytes"
    );
}

#[test]
fn a_short_queue_read_deadline_discards_late_reply_without_stopping_native_work() {
    let mut client = peer("import json,time,sys\nfor line in sys.stdin:\n a=json.loads(line)\n if a['method']=='thread/queue/list': time.sleep(.2)\n print(json.dumps({'id':a['id'],'result':{'data':[]}}))");
    let events = client.take_events_rx().expect("events");
    client
        .request_control_with_context(
            "thread/queue/list",
            json!({"threadId":"root"}),
            json!({"timeoutMs":40,"token":3}),
        )
        .expect("short read");
    let event: Value = serde_json::from_str(
        &events
            .recv_timeout(Duration::from_secs(1))
            .expect("bounded read failure"),
    )
    .expect("JSON");
    assert!(event["params"]["response"]["error"].is_object());
    client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("healthy after read timeout");
    assert!(
        events.try_recv().is_err(),
        "late expired read is not a second result"
    );
}

#[test]
fn an_async_write_flood_hits_the_bounded_pipe_queue_and_stops_the_owned_process() {
    let mut client = peer("import time;time.sleep(60)");
    let events = client.take_events_rx().expect("events");
    let mut failed = false;
    for index in 0..16 {
        let params = json!({"threadId":"root","clientUserMessageId":index.to_string(),"input":[{"type":"text","text":"x".repeat(1024*1024)}]});
        if client
            .request_control_with_context("thread/queue/add", params, json!({"token":index}))
            .is_err()
        {
            failed = true;
            break;
        }
    }
    assert!(failed, "the writer never retains an unbounded burst");
    assert!(client.state.is_closed());
    let mut closed = false;
    while let Ok(line) = events.recv_timeout(Duration::from_secs(1)) {
        if line.contains("carlos/codex/closed") {
            closed = true;
            break;
        }
    }
    assert!(closed, "reserved fatal diagnostic survives the flood");
    client.stop();
    assert!(client.state.reap());
}
