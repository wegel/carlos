//! Stop only a Codex process family created by this client, including wrappers and their children.
#![cfg(target_os = "linux")]

use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::protocol::AppServerClient;

const CHILD_TEST: &str = "protocol::owned_process_tests::stopping_an_owned_wrapper_also_stops_its_child_without_harming_a_bystander";
const FIXTURE_ENV: &str = "CARLOS_TEST_OWNED_PROCESS_GROUP";

#[test]
fn stopping_an_owned_wrapper_also_stops_its_child_without_harming_a_bystander() {
    if std::env::var_os(FIXTURE_ENV).is_some() {
        run_fixture();
        return;
    }
    let status = Command::new(std::env::current_exe().expect("Core test executable"))
        .args(["--exact", CHILD_TEST, "--nocapture"])
        .env(FIXTURE_ENV, "1")
        .status()
        .expect("isolated process supervisor");
    assert!(status.success(), "owned process supervisor failed");
}

struct Bystander(Child);

impl Drop for Bystander {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_fixture() {
    // Only the separately spawned test process becomes a subreaper.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let mut bystander = Bystander(Command::new("sleep").arg("30").spawn().expect("bystander"));
    let mut command = Command::new("python3");
    command.args([
        "-u",
        "-c",
        "import json,subprocess,sys
child=subprocess.Popen(['sleep','30'])
a=json.loads(input())
print(json.dumps({'id':a['id'],'result':{'pid':child.pid}}),flush=True)
sys.stdin.read()",
    ]);
    let client = AppServerClient::start_command(command).expect("owned wrapper");
    let response = client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("child started");
    let response: Value = serde_json::from_str(&response).expect("child identity");
    let child = response["result"]["pid"]
        .as_i64()
        .expect("known descendant pid") as libc::pid_t;
    let started = Instant::now();
    client.stop();
    assert!(started.elapsed() < Duration::from_millis(500));
    let reaped = wait_for_child(child);
    if !reaped {
        // Clean only this captured, still-owned child when checking the pre-fix failure.
        unsafe {
            libc::kill(child, libc::SIGKILL);
            libc::waitpid(child, std::ptr::null_mut(), 0);
        }
    }
    assert!(bystander.0.try_wait().expect("bystander state").is_none());
    assert!(reaped, "stopping a native wrapper left its child alive");
}

fn wait_for_child(child: libc::pid_t) -> bool {
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if unsafe { libc::waitpid(child, std::ptr::null_mut(), libc::WNOHANG) } == child {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}
