//! Keep an actual blocked TCP write charged after the WebSocket library accepts its message.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use tungstenite::{client, Message};

use super::{Frame, Kind};
use crate::network::{self, Accepted, Admissions};
use crate::transport::ingress::Ingress;
use crate::transport::PeerSocket;

#[test]
fn websocket_message_keeps_its_outbox_charge_until_flush_or_connection_teardown() {
    let (_client, peer) = connected_peer(1024, Duration::ZERO);
    assert_charged_until_teardown(peer);
}

#[test]
fn throttled_websocket_reader_keeps_progressing_past_a_whole_frame_deadline() {
    let (mut client, peer) = connected_peer(64 * 1024, Duration::from_millis(40));
    let bytes: Arc<[u8]> = vec![b'x'; 1024 * 1024].into();
    assert!(peer.outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Reply
    }));
    let began = Instant::now();
    let Message::Text(text) = client
        .read()
        .expect("progressing reader must stay connected")
    else {
        panic!("expected the complete text frame");
    };
    assert_eq!(text.as_bytes(), bytes.as_ref());
    assert!(began.elapsed() > Duration::from_millis(500));
    let state = peer.outbox.0 .0.lock().expect("flushed outbox");
    assert_eq!((state.count, state.bytes), (0, 0));
}

struct PacedStream {
    stream: TcpStream,
    delay: Duration,
}

fn connected_peer(
    receive_buffer: usize,
    delay: Duration,
) -> (tungstenite::WebSocket<PacedStream>, PeerSocket) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("bound address");
    let pool = Arc::new(Admissions::default());
    let admission = pool.admit().expect("network slot");
    admission.authorize();
    let (wake, _) = mpsc::sync_channel(1);
    let (opened, accepted) = mpsc::sync_channel(1);
    let owner = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let socket = network::upgrade(stream, None, admission.deadline).expect("upgrade");
        let peer = PeerSocket::start_network(
            1,
            Accepted { socket, admission },
            Ingress::new(wake.clone()),
            wake,
        )
        .expect("peer");
        opened.send(peer).expect("peer handle");
    });
    let stream = TcpStream::connect(address).expect("connect");
    socket2::SockRef::from(&stream)
        .set_recv_buffer_size(receive_buffer)
        .expect("small receive buffer");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("bounded fixture read");
    let (client, _) =
        client(format!("ws://{address}"), PacedStream { stream, delay }).expect("client upgrade");
    let peer = accepted
        .recv_timeout(Duration::from_secs(2))
        .expect("network peer");
    owner.join().expect("upgrade worker");
    (client, peer)
}

impl Read for PacedStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        std::thread::sleep(self.delay);
        let count = bytes.len().min(16 * 1024);
        self.stream.read(&mut bytes[..count])
    }
}

impl Write for PacedStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

fn assert_charged_until_teardown(peer: PeerSocket) {
    let bytes: Arc<[u8]> = vec![b'x'; 4 * 1024 * 1024].into();
    assert!(peer.outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Reply
    }));
    let began = Instant::now();
    loop {
        let state = peer.outbox.0 .0.lock().expect("outbox");
        if state.queue.is_empty() {
            break;
        }
        drop(state);
        assert!(
            began.elapsed() < Duration::from_millis(300),
            "writer must start"
        );
        std::thread::yield_now();
    }
    let until = Instant::now() + Duration::from_millis(100);
    while Instant::now() < until {
        let state = peer.outbox.0 .0.lock().expect("outbox");
        assert_eq!(
            (state.count, state.bytes),
            (1, bytes.len()),
            "a buffered but unflushed message is still charged"
        );
        drop(state);
        std::thread::sleep(Duration::from_millis(2));
    }
    let outbox = peer.outbox.clone();
    drop(peer);
    let state = outbox.0 .0.lock().expect("retired outbox");
    assert_eq!((state.count, state.bytes), (0, 0));
}
