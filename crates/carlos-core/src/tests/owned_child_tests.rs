//! Kill owned descendants before reaping their leader, without signalling an unrelated child.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::owned_child::OwnedChild;

const MODE: &str = "CARLOS_TEST_OWNED_CHILD_MODE";

#[test]
fn direct_stop_reaps_its_descendant_and_keeps_a_bystander_alive() {
    isolated(
        "direct",
        "direct_stop_reaps_its_descendant_and_keeps_a_bystander_alive",
    );
}

#[test]
fn exited_leader_still_retires_its_descendants_before_releasing_its_pid() {
    isolated(
        "exited",
        "exited_leader_still_retires_its_descendants_before_releasing_its_pid",
    );
}

fn isolated(mode: &str, name: &str) {
    if std::env::var(MODE).as_deref() == Ok(mode) {
        fixture(mode);
        return;
    }
    let status = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            &format!("owned_child::tests::{name}"),
            "--nocapture",
        ])
        .env(MODE, mode)
        .status()
        .expect("separate process supervisor");
    assert!(status.success(), "isolated process-family case failed");
}

struct Bystander(Child);

impl Drop for Bystander {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn fixture(mode: &str) {
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let mut bystander = Bystander(Command::new("sleep").arg("30").spawn().expect("bystander"));
    let mut command = Command::new("python3");
    command
        .args(["-u", "-c", SCRIPT, mode])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = OwnedChild::spawn(&mut command).expect("owned family");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("fixture stdout"))
        .read_line(&mut line)
        .expect("descendant ready");
    let descendant = line
        .trim()
        .parse::<libc::pid_t>()
        .expect("captured descendant");
    if mode == "direct" {
        child.kill().expect("close owned family");
    }
    let status = wait_leader(&mut child);
    assert_eq!(status.success(), mode == "exited");
    assert_eq!(
        child.try_wait().expect("cached reaped status"),
        Some(status)
    );
    child
        .kill()
        .expect("repeated cleanup cannot signal a reused ID");
    let reaped = wait_descendant(descendant);
    if !reaped && unsafe { libc::waitpid(descendant, std::ptr::null_mut(), libc::WNOHANG) } == 0 {
        unsafe {
            libc::kill(descendant, libc::SIGKILL);
            libc::waitpid(descendant, std::ptr::null_mut(), 0);
        }
    }
    assert!(bystander.0.try_wait().expect("bystander state").is_none());
    assert!(reaped, "owned descendant survived cleanup");
}

fn wait_leader(child: &mut OwnedChild) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().expect("observe without early reaping") {
            return status;
        }
        assert!(Instant::now() < deadline, "leader must exit");
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_descendant(pid: libc::pid_t) -> bool {
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) } == pid {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

const SCRIPT: &str = "import os,subprocess,sys\nchild=subprocess.Popen(['sleep','30'])\nprint(child.pid,flush=True)\nif sys.argv[1]=='exited': os._exit(0)\nsys.stdin.read()\n";
