//! Exercise simultaneous resize and keyboard readiness through the real Unix terminal reader.

#![cfg(unix)]

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode};

const CONTROL: &str = "CARLOS_RESIZE_INPUT_CONTROL";

#[test]
fn resize_keeps_an_already_readable_key_without_another_input_wakeup() {
    // Each fresh reader tests the same simultaneous signal/input batch. There is no
    // later input to rescue a readiness edge dropped while returning the resize.
    for _ in 0..8 {
        let directory = tempfile::tempdir().expect("private control directory");
        let path = directory.path().join("control");
        let listener = UnixListener::bind(&path).expect("control listener");
        let (master, slave) = terminal_pair();
        let mut child = spawn_probe(&path, &slave);
        drop(slave);
        let events = collect_probe(&listener, master);
        let status = child.wait().expect("probe exit");
        assert!(status.success(), "terminal probe failed: {events:?}");
        assert!(events.iter().any(|line| line == "resize"), "{events:?}");
        assert!(events.iter().any(|line| line == "end"), "{events:?}");
    }
}

#[test]
fn terminal_resize_probe_child() {
    let Some(path) = std::env::var_os(CONTROL) else {
        return;
    };
    let mut control = UnixStream::connect(path).expect("control connection");
    control
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("control timeout");
    crossterm::terminal::enable_raw_mode().expect("raw terminal");
    assert!(!event::poll(Duration::from_millis(1)).expect("initialize reader"));
    // raise returns only after Crossterm's registered handler has queued SIGWINCH.
    assert_eq!(unsafe { libc::raise(libc::SIGWINCH) }, 0);
    control.write_all(b"ready\n").expect("reader ready");
    control.read_exact(&mut [0]).expect("keyboard bytes queued");
    let deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < deadline {
        if event::poll(Duration::from_millis(8)).expect("poll terminal") {
            let name = match event::read().expect("read terminal") {
                Event::Resize(_, _) => "resize",
                Event::Key(key) if key.code == KeyCode::End => "end",
                _ => "other",
            };
            writeln!(control, "{name}").expect("event report");
        }
    }
    crossterm::terminal::disable_raw_mode().expect("restore terminal");
    control.write_all(b"finished\n").expect("finished report");
}

fn terminal_pair() -> (File, File) {
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: 32,
        ws_col: 28,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // openpty initializes both owned descriptors on success; each File closes one.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            &size,
        )
    };
    assert_eq!(
        result,
        0,
        "open terminal: {}",
        std::io::Error::last_os_error()
    );
    unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) }
}

fn spawn_probe(path: &std::path::Path, slave: &File) -> Child {
    Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "terminal_resize_probe_child", "--nocapture"])
        .env(CONTROL, path)
        .stdin(Stdio::from(slave.try_clone().expect("terminal input")))
        .stdout(Stdio::from(slave.try_clone().expect("terminal output")))
        .stderr(Stdio::from(slave.try_clone().expect("terminal errors")))
        .spawn()
        .expect("terminal probe")
}

fn collect_probe(listener: &UnixListener, mut master: File) -> Vec<String> {
    let mut descriptor = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(
        unsafe { libc::poll(&mut descriptor, 1, 3000) },
        1,
        "probe connection"
    );
    let (control, _) = listener.accept().expect("accept control");
    control
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("report timeout");
    let mut control = BufReader::new(control);
    let mut line = String::new();
    control.read_line(&mut line).expect("ready report");
    assert_eq!(line, "ready\n");
    master.write_all(b"\x1b[F").expect("queue End");
    control.get_mut().write_all(b"x").expect("release reader");
    let mut events = Vec::new();
    loop {
        line.clear();
        assert!(control.read_line(&mut line).expect("event report") > 0);
        if line == "finished\n" {
            return events;
        }
        events.push(line.trim_end().to_owned());
    }
}
