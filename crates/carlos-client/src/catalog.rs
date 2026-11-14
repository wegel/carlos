//! Bound session-list facts before replacing or extending a device's catalogue.

use std::collections::HashMap;
use std::io::{self, Write};

use carlos_proto::ids::SessionId;
use carlos_proto::session::SessionInfo;

use crate::ClientError;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_SESSIONS: usize = 4096;

#[derive(Default)]
pub(crate) struct Catalog {
    entries: HashMap<SessionId, (SessionInfo, usize)>,
    bytes: usize,
}

impl Catalog {
    pub fn iter(&self) -> impl Iterator<Item = &SessionInfo> {
        self.entries.values().map(|(info, _)| info)
    }

    pub fn replace(&mut self, entries: Vec<SessionInfo>) -> Result<(), ClientError> {
        if entries.len() > MAX_SESSIONS {
            return Err(ClientError::Capacity("session catalogue is full"));
        }
        let mut next = Self::default();
        for entry in entries {
            if next.entries.contains_key(&entry.id) {
                return Err(ClientError::InvalidPayload(
                    "duplicate catalogue session".into(),
                ));
            }
            next.update(entry)?;
        }
        *self = next;
        Ok(())
    }

    pub fn update(&mut self, info: SessionInfo) -> Result<(), ClientError> {
        let mut counter = Counter(0);
        serde_json::to_writer(&mut counter, &info)
            .map_err(|_| ClientError::Capacity("session catalogue exceeds four MiB"))?;
        let old = self.entries.get(&info.id).map_or(0, |(_, bytes)| *bytes);
        let bytes = self.bytes - old + counter.0;
        if bytes > MAX_BYTES
            || !self.entries.contains_key(&info.id) && self.entries.len() >= MAX_SESSIONS
        {
            return Err(ClientError::Capacity("session catalogue is full"));
        }
        self.entries.insert(info.id.clone(), (info, counter.0));
        self.bytes = bytes;
        Ok(())
    }

    pub fn remove(&mut self, session: &SessionId) {
        if let Some((_, bytes)) = self.entries.remove(session) {
            self.bytes -= bytes;
        }
    }
}

struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.0) {
            return Err(io::Error::other("catalogue byte cap"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
