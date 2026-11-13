//! Controls a live helper after stdout EOF through a separate inherited socket.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde_json::json;

use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{pause_once, receive, transport, Pause, DEADLINE};
use super::event_test_support::{command_client, records, submit};
use super::pipe_test_support::inherit_descriptor;
use super::transport::ControllerTransport;
use super::ClaudeClient;
use crate::backend::EventReceiver;

pub(super) struct EofPeer {
    pub(super) client: ClaudeClient,
    pub(super) receiver: EventReceiver,
    pub(super) id: String,
    control: UnixStream,
}

#[derive(Clone, Copy)]
pub(super) enum FullQueue {
    Bytes,
    Frames,
}

impl EofPeer {
    pub(super) fn start() -> Self {
        Self::start_with_descriptor(false)
    }

    pub(super) fn start_with_descriptor(force_same_descriptor: bool) -> Self {
        let (control, helper) = UnixStream::pair().expect("control socket");
        control.set_read_timeout(Some(DEADLINE)).expect("deadline");
        let fd = helper.as_raw_fd();
        let mut command = Command::new("python3");
        command.args(["-c", PEER]);
        // Only the child inherits descriptor 100; unrelated concurrent peers cannot hold it open.
        unsafe {
            command.pre_exec(move || {
                let source = if force_same_descriptor {
                    // Reproduce a socket created at 100 with close-on-exec still set.
                    if libc::dup2(fd, 100) == -1
                        || libc::fcntl(100, libc::F_SETFD, libc::FD_CLOEXEC) == -1
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    100
                } else {
                    fd
                };
                inherit_descriptor(source, 100)
            });
        }
        let (client, receiver) = command_client(command, 1024);
        drop(helper);
        let id = submit(&client);
        records(&client, &receiver);
        Self {
            client,
            receiver,
            id,
            control,
        }
    }

    pub(super) fn signal(&mut self, byte: u8) {
        self.control.write_all(&[byte]).expect("release helper");
        let mut ack = [0];
        self.control
            .read_exact(&mut ack)
            .expect("helper reached barrier");
        assert_eq!(ack, [byte]);
    }

    pub(super) fn close_stdout_with_pending_call(&mut self) -> Pause {
        let transport = transport(&self.client);
        let calling = transport.clone();
        let (done_tx, done) = mpsc::channel();
        let caller = thread::spawn(move || {
            let result = calling.call("pending", json!({}), DEADLINE);
            done_tx.send(result).expect("pending result");
        });
        let marker = receive(&transport.events, &self.receiver);
        assert_eq!(marker["params"]["callbackId"], "pending");
        let eof = pause_once(&transport.events.hooks, "reader_eof");
        self.signal(b'E');
        eof.wait();
        assert!(transport
            .child
            .lock()
            .expect("child")
            .try_wait()
            .expect("alive")
            .is_none());
        assert_eq!(
            done.recv_timeout(Duration::from_secs(1))
                .expect("call rejected before final delivery")
                .expect_err("closed stdout")
                .to_string(),
            "Claude controller closed before replying"
        );
        caller.join().expect("caller joined");
        assert!(transport.pending.lock().expect("pending").is_empty());
        assert_closed(&transport);
        eof
    }
}

impl FullQueue {
    pub(super) fn fill(self, peer: &EofPeer) {
        for _ in 0..self.count() {
            peer.client
                .events_tx
                .send(self.filler())
                .expect("fill queue");
        }
    }

    pub(super) fn drain(self, peer: &EofPeer) {
        let filler = self.filler();
        for _ in 0..self.count() {
            let line = peer.receiver.recv_timeout(DEADLINE).expect("filler");
            assert_eq!(line, filler);
            peer.client.events_tx.handled(line.len());
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Bytes => 1,
            Self::Frames => 1024,
        }
    }

    fn filler(self) -> String {
        match self {
            Self::Bytes => "x".repeat(MAX_EVENT_BYTES),
            Self::Frames => "filler".to_string(),
        }
    }
}

pub(super) fn assert_closed(transport: &ControllerTransport) {
    assert_eq!(
        transport
            .call("later", json!({}), DEADLINE)
            .expect_err("closed as on main")
            .to_string(),
        "Claude controller is closed"
    );
}

pub(super) fn assert_reaped(transport: &Arc<ControllerTransport>) {
    assert_closed(transport);
    assert!(transport
        .child
        .lock()
        .expect("child")
        .try_wait()
        .expect("child status")
        .is_some());
    assert_eq!(transport.state.shutdown_requests.load(Ordering::SeqCst), 1);
}

const PEER: &str = r#"
import sys,json,os,socket,threading
control=socket.socket(fileno=100)
def controls():
 while True:
  byte=control.recv(1)
  if not byte: return
  if byte==b'E': os.close(1)
  control.sendall(byte)
  # stderr is diagnostic text on main, including a protocol-error diagnostic.
  if byte in [b'S',b'P']:
   text='protocol error: invalid controller frame' if byte==b'P' else 'stderr after EOF'
   print(text,file=sys.stderr,flush=True)
threading.Thread(target=controls,daemon=True).start()
for line in sys.stdin:
 r=json.loads(line)
 if r['method']=='pending':
  print(json.dumps(dict(v=1,type='event',event='callback_cancelled',callbackId='pending')),flush=True)
  continue
 print(json.dumps(dict(v=1,type='reply',id=r['id'],ok=True,result={})),flush=True)
 if r['method']=='shutdown': break
"#;
