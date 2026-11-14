//! Reject oversized or stalled partial frames without putting socket reads on the manager.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::ingress::Ingress;
use super::outbox::{Frame, Kind, Outbox};
use super::{ready, wire};

pub(super) fn run(peer: u64, mut stream: &UnixStream, ingress: Ingress, outbox: &Outbox) {
    let mut buffer = [0u8; 16384];
    let mut partial = Vec::new();
    let mut began = None;
    loop {
        if began.is_some_and(|start: Instant| start.elapsed() > Duration::from_secs(5)) {
            return;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => {
                if !consume(peer, &buffer[..count], &mut partial, &ingress, outbox) {
                    return;
                }
                if partial.is_empty() {
                    began = None;
                } else if began.is_none() {
                    began = Some(Instant::now());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if ready(stream, libc::POLLIN, Duration::from_millis(250)).is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

fn consume(
    peer: u64,
    bytes: &[u8],
    partial: &mut Vec<u8>,
    ingress: &Ingress,
    outbox: &Outbox,
) -> bool {
    for segment in bytes.split_inclusive(|byte| *byte == b'\n') {
        if segment.len() > crate::limits::REQUEST_BYTES.saturating_sub(partial.len()) {
            return false;
        }
        partial.extend_from_slice(segment);
        if segment.last() != Some(&b'\n') {
            continue;
        }
        let accepted = match wire::decode(partial) {
            Ok(request) => ingress.push(peer, request, partial.len()),
            Err(error) => wire::encode(&error).is_ok_and(|bytes| {
                outbox.push(Frame {
                    bytes,
                    kind: Kind::Reply,
                })
            }),
        };
        partial.clear();
        if !accepted {
            return false;
        }
    }
    true
}
