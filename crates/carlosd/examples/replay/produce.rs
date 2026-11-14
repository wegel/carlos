//! Export the same sealed history and output reads used by session owners, without UI work.

use std::sync::mpsc;
use std::time::Instant;

use anyhow::Result;
use carlos_core::session::events::Published;
use carlos_core::session::models::Role;
use carlos_core::session::state::SessionState;
use carlos_proto::ids::EntryId;

use super::Packet;

pub(super) fn pages(mut session: SessionState, sender: mpsc::SyncSender<Vec<u8>>) -> Result<f64> {
    let tools: Vec<_> = session
        .messages()
        .iter()
        .filter(|row| row.role == Role::ToolOutput)
        .map(|row| row.id.to_wire())
        .collect();
    let start = Instant::now();
    let published = session.publish();
    let snapshot = published.snapshot(100);
    let mut before = snapshot.entries.first().map(|row| row.id.clone());
    let mut elapsed = emit(Packet::Snapshot(snapshot), &sender, start)?;
    while let Some(id) = before {
        let start = Instant::now();
        let history = published.history(&id, 100).expect("stable source cursor");
        before = history
            .entries
            .first()
            .filter(|_| history.has_older)
            .map(|row| row.id.clone());
        elapsed += emit(Packet::History(history), &sender, start)?;
    }
    for entry in tools {
        elapsed += output(&published, entry, &sender)?;
    }
    Ok(elapsed)
}

fn output(
    published: &Published<'_>,
    entry: EntryId,
    sender: &mpsc::SyncSender<Vec<u8>>,
) -> Result<f64> {
    let mut cursor = None;
    let mut elapsed = 0.0;
    loop {
        let start = Instant::now();
        let chunk = published.output_chunk(&entry, cursor.as_ref(), 65536)?;
        cursor = Some(chunk.next.clone());
        let more = chunk.has_more;
        elapsed += emit(Packet::Output(chunk), sender, start)?;
        if !more {
            return Ok(elapsed);
        }
    }
}

fn emit(packet: Packet, sender: &mpsc::SyncSender<Vec<u8>>, start: Instant) -> Result<f64> {
    let mut frame = serde_json::to_vec(&packet)?;
    frame.push(b'\n');
    assert!(frame.len() <= carlosd::limits::FRAME_BYTES);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    sender.send(frame)?;
    Ok(elapsed)
}
