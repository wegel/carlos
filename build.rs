//! Stamp the terminal package without putting workspace reads in its data dependencies.

#[path = "scripts/source_fingerprint.rs"]
mod source_fingerprint;

fn main() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"),
    );
    source_fingerprint::emit(&root);
}
