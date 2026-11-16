//! Keep the daemon's peer, command, replay and recovery budgets explicit.

/// Maximum source bytes in one incoming JSON-RPC frame.
pub const REQUEST_BYTES: usize = 4 * 1024 * 1024;
/// Maximum encoded bytes in one outgoing JSON-RPC frame, including its newline.
pub const FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Maximum source text in one human send.
pub const SEND_BYTES: usize = 512 * 1024;
/// Maximum UTF-8 bytes in a catalogue name or explicit rename request.
pub const NAME_BYTES: usize = 4096;
/// Maximum simultaneously connected device sockets.
pub const PEERS: usize = 16;
/// Network connections never consume the sixteen trusted Unix peer slots.
pub const NETWORK_PEERS: usize = 16;
/// Bound HTTP upgrades and clients awaiting their first authenticated greeting together.
pub const NETWORK_UNAUTHENTICATED: usize = 4;
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
/// Reserve 32 frame slots for replies and recovery even when shared events are small.
pub const SHARED_FRAMES: usize = 96;
/// Maximum queued and executing socket-write bytes per peer.
pub const OUT_BYTES: usize = 16 * 1024 * 1024;
/// Leave room beyond ordinary shared facts for an explicit replacement snapshot.
pub const SHARED_BYTES: usize = 8 * 1024 * 1024;
/// Bound outstanding owner replies as well as the small commands that requested them.
pub const READ_BYTES: usize = 32 * 1024 * 1024;
