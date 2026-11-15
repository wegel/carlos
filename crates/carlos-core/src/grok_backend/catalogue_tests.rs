//! Reject named pipes before a local metadata fallback can occupy a lifecycle worker forever.
#![cfg(unix)]

use std::ffi::CString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::read_summary;

#[test]
fn a_named_pipe_is_not_a_native_summary_file() {
    if let Some(path) = std::env::var_os("CARLOS_TEST_GROK_SUMMARY_PIPE") {
        let path = Path::new(&path);
        assert!(
            read_summary(path, path.parent().expect("metadata directory"))
                .expect("metadata read")
                .is_none()
        );
        return;
    }
    let root = tempfile::tempdir().expect("private Grok metadata");
    let path = root.path().join("summary.json");
    let name = CString::new(path.as_os_str().as_encoded_bytes()).expect("FIFO path");
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "grok_backend::catalogue::tests::a_named_pipe_is_not_a_native_summary_file",
        ])
        .env("CARLOS_TEST_GROK_SUMMARY_PIPE", path)
        .stdout(Stdio::null())
        .spawn()
        .expect("bounded metadata subprocess");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().expect("metadata child status") {
            assert!(status.success(), "metadata child failed");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("named-pipe metadata waited for a writer");
        }
        thread::sleep(Duration::from_millis(5));
    }
}
