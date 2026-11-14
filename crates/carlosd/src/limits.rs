//! Keep the daemon's peer, command, replay and recovery budgets explicit.

/// Maximum source bytes in one incoming JSON-RPC frame.
pub const REQUEST_BYTES: usize = 4 * 1024 * 1024;
/// Maximum encoded bytes in one outgoing JSON-RPC frame, including its newline.
pub const FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Maximum source text in one human send.
pub const SEND_BYTES: usize = 512 * 1024;
/// Maximum simultaneously connected device sockets.
pub const PEERS: usize = 16;
/// Maximum live native session owners.
pub const OWNERS: usize = 16;
/// Maximum saved session descriptions, including dormant and unavailable ones.
pub const SESSIONS: usize = 256;
/// Maximum session interests retained by one device connection.
pub const INTERESTS: usize = 16;
/// Maximum expanded output watches retained by one device connection.
pub const WATCHES: usize = 32;
/// Maximum queued and executing socket-write frames per peer.
pub const OUT_FRAMES: usize = 128;
/// Maximum queued and executing socket-write bytes per peer.
pub const OUT_BYTES: usize = 16 * 1024 * 1024;
/// Leave room beyond ordinary shared facts for an explicit replacement snapshot.
pub const SHARED_BYTES: usize = 8 * 1024 * 1024;
