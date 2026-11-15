//! Holds inherited pipes behind socket barriers and reaps each fixture's grandchild.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde_json::json;

use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{notify, transport, DEADLINE};
use super::event_test_support::command_client;
use super::pipe_test_support::inherit_descriptor;
use super::transport::ControllerTransport;
use super::ClaudeClient;
use crate::backend::EventReceiver;

pub(super) struct PipePeer {
    pub(super) client: Option<ClaudeClient>,
    pub(super) transport: Arc<ControllerTransport>,
    _receiver: EventReceiver,
    holder: PipeHolder,
    caller: Option<JoinHandle<()>>,
}

struct PipeHolder {
    control: UnixStream,
    pid: libc::pid_t,
}

impl PipePeer {
    pub(super) fn start(pipe: &str, full_budget: bool) -> Self {
        // Orphans belong to this test process, so cleanup can kill AND reap its exact PID.
        assert_eq!(unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1) }, 0);
        let (control, helper) = UnixStream::pair().expect("control socket");
        control.set_read_timeout(Some(DEADLINE)).expect("deadline");
        let fd = helper.as_raw_fd();
        let mut command = Command::new("python3");
        command.args(["-c", PEER, pipe]);
        unsafe {
            command.pre_exec(move || inherit_descriptor(fd, 100));
        }
        let (client, receiver) = command_client(command, 1024);
        drop(helper);
        let transport = transport(&client);
        let mut holder = PipeHolder { control, pid: 0 };
        let mut pid = [0; 4];
        holder.control.read_exact(&mut pid).expect("holder ready");
        holder.pid = i32::from_ne_bytes(pid);
        if full_budget {
            transport
                .events
                .send("x".repeat(MAX_EVENT_BYTES))
                .expect("full byte budget");
        }
        let mut peer = Self {
            client: Some(client),
            transport,
            _receiver: receiver,
            holder,
            caller: None,
        };
        if pipe == "stdin" {
            peer.fill_stdin();
        }
        peer
    }

    fn fill_stdin(&mut self) {
        let transport = self.transport.clone();
        self.caller = Some(thread::spawn(move || {
            let result = transport.request("large", json!({"text":"x".repeat(2 * 1024 * 1024)}));
            assert!(result.is_err(), "holder never reads the request");
        }));
        self.holder.expect(b'F');
    }

    pub(super) fn exit_helper(&mut self, pipe: &str) {
        let eof = (pipe != "stdout").then(|| notify(&self.transport.events.hooks, "reader_eof"));
        self.holder.control.write_all(b"E").expect("exit helper");
        self.holder.expect(b'E');
        if let Some(eof) = eof {
            eof.recv_timeout(DEADLINE)
                .expect("stdout closed before drop");
        }
    }

    pub(super) fn cleanup(&mut self) {
        // Also cover a panic before the explicit exit barrier; adoption precedes waitpid.
        if let Ok(mut child) = self.transport.child.lock() {
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.holder.kill();
        if let Some(caller) = self.caller.take() {
            caller
                .join()
                .expect("request released by closing the held pipe");
        }
    }
}

impl Drop for PipePeer {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl PipeHolder {
    fn expect(&mut self, byte: u8) {
        let mut ack = [0];
        self.control.read_exact(&mut ack).expect("holder barrier");
        assert_eq!(ack, [byte]);
    }

    fn kill(&mut self) {
        if self.pid == 0 {
            return;
        }
        unsafe {
            assert_eq!(libc::kill(self.pid, libc::SIGKILL), 0, "kill holder");
            let mut status = 0;
            loop {
                let result = libc::waitpid(self.pid, &mut status, 0);
                if result == self.pid {
                    break;
                }
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EINTR)
                );
            }
        }
        self.pid = 0;
    }
}

impl Drop for PipeHolder {
    fn drop(&mut self) {
        self.kill();
    }
}

const PEER: &str = r#"
import array,fcntl,json,os,socket,struct,sys,termios
control=socket.socket(fileno=100)
mode=sys.argv[1]
if mode=='ordinary':
 control.sendall(struct.pack('i',0))
 for line in sys.stdin:
  if json.loads(line)['method']=='shutdown': break
 sys.exit(0)
exit_read,exit_write=os.pipe()
parent,holder=socket.socketpair()
pid=os.fork()
if pid:
 os.close(exit_write); holder.close(); control.close()
 os.read(exit_read,1)
 os._exit(0)
# This holder intentionally escapes the helper's owned process group. Dropping
# Carlos must stay bounded even when an unowned process retains an inherited pipe.
os.setsid()
os.close(exit_read); parent.close()
keep={'stdin':0,'stdout':1,'stderr':2}[mode]
for fd in range(3):
 if fd!=keep: os.close(fd)
control.sendall(struct.pack('i',os.getpid()))
if mode=='stdin':
 capacity=fcntl.fcntl(0,fcntl.F_GETPIPE_SZ)
 assert capacity < 2*1024*1024
 count=array.array('i',[0])
 while True:
  fcntl.ioctl(0,termios.FIONREAD,count,True)
  if count[0]==capacity: break
 control.sendall(b'F')
assert control.recv(1)==b'E'
os.write(exit_write,b'E'); os.close(exit_write)
assert holder.recv(1)==b''
control.sendall(b'E')
# Never read the held pipe. Rust cleanup kills and reaps this process after drop.
control.recv(1)
os._exit(0)
"#;
