//! Hash native task values without allocating a second encoded metadata body.

use std::io::{self, Write};

use serde_json::Value;

pub(in crate::session) fn hash_text(hash: &mut blake3::Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

pub(in crate::session) fn hash_value(hash: &mut blake3::Hasher, value: &Value) {
    struct Digest<'a>(&'a mut blake3::Hasher);
    impl Write for Digest<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Digest(hash), value).expect("native JSON value");
    hash.update(b"\0");
}
