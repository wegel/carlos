//! Stamp the daemon package with the same source marker as its terminal companion.

#[path = "../../scripts/source_fingerprint.rs"]
mod source_fingerprint;

fn main() {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"),
    );
    source_fingerprint::emit(
        manifest
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace"),
    );
}
