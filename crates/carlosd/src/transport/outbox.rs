//! Charge queued and active writes together, and discard only explicitly recoverable streams.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};

use carlos_proto::ids::{EntryId, SessionId};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Reply,
    Shared(SessionId),
    Snapshot(SessionId),
    Output(SessionId, EntryId),
    Source(SessionId, EntryId),
}

pub(crate) struct Frame {
    pub bytes: Arc<[u8]>,
    pub kind: Kind,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Frame>,
    count: usize,
    bytes: usize,
    closed: bool,
    wake: Option<UnixStream>,
}

#[derive(Clone, Default)]
pub(crate) struct Outbox(Arc<(Mutex<State>, Condvar)>);

impl Outbox {
    pub(crate) fn with_wake(wake: UnixStream) -> io::Result<Self> {
        wake.set_nonblocking(true)?;
        let state = State {
            wake: Some(wake),
            ..State::default()
        };
        Ok(Self(Arc::new((Mutex::new(state), Condvar::new()))))
    }

    pub(crate) fn wake(&self) {
        self.0 .0.lock().expect("peer output queue").notify();
    }

    pub(crate) fn push(&self, frame: Frame) -> bool {
        let mut state = self.0 .0.lock().expect("peer output queue");
        let shared = matches!(
            frame.kind,
            Kind::Shared(_) | Kind::Output(..) | Kind::Source(..)
        );
        let (frames, bytes) = if shared {
            (crate::limits::SHARED_FRAMES, crate::limits::SHARED_BYTES)
        } else {
            (crate::limits::OUT_FRAMES, crate::limits::OUT_BYTES)
        };
        if state.closed
            || state.count >= frames
            || frame.bytes.len() > bytes.saturating_sub(state.bytes)
        {
            return false;
        }
        state.count += 1;
        state.bytes += frame.bytes.len();
        state.queue.push_back(frame);
        self.0 .1.notify_one();
        state.notify();
        true
    }

    pub(crate) fn take(&self) -> Option<Frame> {
        let mut state = self.0 .0.lock().expect("peer output queue");
        loop {
            if state.closed {
                return None;
            }
            if let Some(frame) = state.queue.pop_front() {
                return Some(frame);
            }
            state = self.0 .1.wait(state).expect("peer output wake");
        }
    }

    pub(crate) fn try_take(&self) -> Option<Frame> {
        let mut state = self.0 .0.lock().expect("peer output queue");
        if state.closed {
            None
        } else {
            state.queue.pop_front()
        }
    }

    pub(crate) fn finish(&self, frame: &Frame) {
        let mut state = self.0 .0.lock().expect("peer output queue");
        state.count -= 1;
        state.bytes -= frame.bytes.len();
    }

    pub(crate) fn discard(&self, mut matches: impl FnMut(&Kind) -> bool) {
        let mut state = self.0 .0.lock().expect("peer output queue");
        let mut count = 0;
        let mut bytes = 0;
        state.queue.retain(|frame| {
            if !matches(&frame.kind) {
                return true;
            }
            count += 1;
            bytes += frame.bytes.len();
            false
        });
        state.count -= count;
        state.bytes -= bytes;
    }

    pub(crate) fn close(&self) {
        let mut state = self.0 .0.lock().expect("peer output queue");
        state.closed = true;
        while let Some(frame) = state.queue.pop_front() {
            state.count -= 1;
            state.bytes -= frame.bytes.len();
        }
        self.0 .1.notify_all();
        state.notify();
    }
}

impl State {
    fn notify(&self) {
        if let Some(mut stream) = self.wake.as_ref() {
            let _ = stream.write(&[1]);
        }
    }
}

#[cfg(test)]
#[path = "websocket_write_tests.rs"]
mod websocket_write_tests;
