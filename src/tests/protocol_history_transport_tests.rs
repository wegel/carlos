//! Keep batched history off the UI stream without widening normal native budgets.

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol::AppServerClient;

fn peer(script: &str) -> AppServerClient {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", script]);
    AppServerClient::start_command(command).expect("isolated pipe peer")
}

#[test]
fn full_history_pages_and_legacy_full_reads_can_exceed_the_normal_wire_budget() {
    for (method, params) in [
        ("thread/turns/list", json!({"itemsView":"full"})),
        ("thread/items/list", json!({"turnId":"turn"})),
        ("thread/read", json!({"includeTurns":true})),
    ] {
        let mut client = peer("import json,sys\na=json.loads(input()); print(json.dumps({'result':{'text':'x'*(9*1024*1024)},'id':a['id']})); sys.stdin.read()");
        let events = client.take_events_rx().expect("events");
        let result = client
            .call(method, params, Duration::from_secs(4))
            .expect("large history reply");
        assert_eq!(
            serde_json::from_str::<Value>(&result).expect("JSON")["result"]["text"]
                .as_str()
                .expect("text")
                .len(),
            9 * 1024 * 1024
        );
        assert!(
            events.try_recv().is_err(),
            "history must never enter the UI event queue"
        );
    }
}

#[test]
fn late_large_history_replies_drain_without_closing_healthy_native_work() {
    let mut client = peer("import json,time,sys\na=json.loads(input()); time.sleep(.15); print(json.dumps({'id':a['id'],'result':'x'*(9*1024*1024)}))\nb=json.loads(input()); print(json.dumps({'id':b['id'],'result':{'ok':True}})); sys.stdin.read()");
    let events = client.take_events_rx().expect("events");
    assert!(client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_millis(30)
        )
        .is_err());
    let response = client
        .call("model/list", json!({}), Duration::from_secs(4))
        .expect("healthy next read");
    assert_eq!(
        serde_json::from_str::<Value>(&response).expect("JSON")["result"]["ok"],
        true
    );
    assert!(events.try_recv().is_err());
}

#[test]
fn unrelated_replies_and_events_keep_the_ordinary_eight_mib_cap() {
    for body in [
        "{'id':a['id'],'result':'x'*(9*1024*1024)}",
        "{'id':'unissued','result':'x'*(9*1024*1024)}",
        "{'method':'future/event','params':{'text':'x'*(9*1024*1024)}}",
    ] {
        let script = format!(
            "import json,sys\na=json.loads(input()); print(json.dumps({body})); sys.stdin.read()"
        );
        let client = peer(&script);
        let method = if body.contains("a['id']") {
            "model/list"
        } else {
            "thread/turns/list"
        };
        let error = client
            .call(method, json!({"itemsView":"full"}), Duration::from_secs(4))
            .expect_err("ordinary oversized frame closes");
        assert!(error.to_string().contains("8 MiB"), "{error}");
    }
}

#[test]
fn history_reply_still_has_a_hard_sixty_four_mib_ceiling() {
    let client = peer("import sys\ninput(); sys.stdout.write('x'*(64*1024*1024+1)); sys.stdout.flush(); sys.stdin.read()");
    let error = client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(4),
        )
        .expect_err("bounded read");
    assert!(error.to_string().contains("exceeds 64 MiB"), "{error}");
}

#[test]
fn unanswered_large_read_ids_stay_bounded_and_do_not_disable_other_controls() {
    let client = peer("import json,sys\npending=[]; drained=False\nfor line in sys.stdin:\n a=json.loads(line)\n if a['method']=='thread/turns/list' and not drained: pending.append(a); continue\n for old in pending: print(json.dumps({'id':old['id'],'result':{}}))\n pending=[]; drained=True\n print(json.dumps({'id':a['id'],'result':{}}))");
    for _ in 0..64 {
        let error = client
            .call(
                "thread/turns/list",
                json!({"itemsView":"full"}),
                Duration::from_millis(20),
            )
            .expect_err("read times out");
        assert!(error.to_string().contains("Timeout waiting"), "{error}");
    }
    let error = client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(2),
        )
        .expect_err("bounded drain IDs");
    assert!(
        error.to_string().contains("64 undrained history reads"),
        "{error}"
    );
    client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("ordinary controls still work and drain retired replies");
    client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(2),
        )
        .expect("drained allowance recovers");
}

#[test]
fn metadata_and_asynchronous_history_replies_never_get_a_large_read_allowance() {
    let script = "import json,sys\na=json.loads(input()); print(json.dumps({'id':a['id'],'result':'x'*(9*1024*1024)})); sys.stdin.read()";
    for (method, params) in [
        ("thread/read", json!({"includeTurns":false})),
        ("thread/turns/list", json!({"itemsView":"notLoaded"})),
    ] {
        let client = peer(script);
        let error = client
            .call(method, params, Duration::from_secs(4))
            .expect_err("metadata keeps ordinary cap");
        assert!(error.to_string().contains("8 MiB"), "{error}");
    }
    let mut client = peer(script);
    let events = client.take_events_rx().expect("events");
    client
        .request_native_control("thread/turns/list", json!({"itemsView":"full"}))
        .expect("async request");
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(4))
            .expect("bounded failure");
        if event.contains("carlos/codex/closed") {
            assert!(event.contains("8 MiB"));
            break;
        }
    }
}

#[test]
fn only_one_large_history_call_can_wait_while_normal_controls_keep_working() {
    let mut client = peer("import json,sys\na=json.loads(input()); print(json.dumps({'method':'future/accepted','params':{}}))\nb=json.loads(input()); print(json.dumps({'id':a['id'],'result':{}})); print(json.dumps({'id':b['id'],'result':{}})); sys.stdin.read()");
    let events = client.take_events_rx().expect("events");
    let client = std::sync::Arc::new(client);
    let waiting = client.clone();
    let first = std::thread::spawn(move || {
        waiting.call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(4),
        )
    });
    events
        .recv_timeout(Duration::from_secs(2))
        .expect("first request arrived");
    let error = client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(2),
        )
        .expect_err("one large call");
    assert!(error
        .to_string()
        .contains("already has an active large history read"));
    client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("ordinary control remains responsive");
    first
        .join()
        .expect("worker joined")
        .expect("first read settles");
}

#[test]
fn a_locally_rejected_write_releases_its_unused_history_reply_allowance() {
    let client = peer("import json,sys\na=json.loads(input()); print(json.dumps({'id':a['id'],'result':{}})); sys.stdin.read()");
    let error = client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full","unused":"x".repeat(8 * 1024 * 1024)}),
            Duration::from_secs(3),
        )
        .expect_err("request is too large before dispatch");
    assert!(error.to_string().contains("8 MiB"), "{error}");
    assert!(
        !client
            .state
            .finish_large_reply(&json!("carlos-rpc-1").to_string()),
        "unsent request cannot keep a large-reply grant"
    );
    client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(2),
        )
        .expect("next history request works");
}
