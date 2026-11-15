//! Give colocated executables the same deterministic source compatibility marker.

use std::fs;
use std::path::{Path, PathBuf};

const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

/// Publish one marker from sorted relative source names and bytes for a binary package.
pub fn emit(root: &Path) {
    let mut inputs = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("build.rs"),
        root.join("scripts/source_fingerprint.rs"),
    ];
    collect(&root.join("src"), &mut inputs);
    for package in fs::read_dir(root.join("crates")).expect("workspace crates") {
        let package = package.expect("crate directory").path();
        inputs.push(package.join("Cargo.toml"));
        if package.join("build.rs").is_file() {
            inputs.push(package.join("build.rs"));
        }
        collect(&package.join("src"), &mut inputs);
    }
    let helper = root.join("helpers/claude-controller");
    collect(&helper.join("src"), &mut inputs);
    for name in ["package.json", "bun.lock", "bunfig.toml", "tsconfig.json"] {
        inputs.push(helper.join(name));
    }
    inputs.sort();
    let mut hash = OFFSET;
    for input in inputs {
        println!("cargo:rerun-if-changed={}", input.display());
        let relative = input.strip_prefix(root).expect("relative input");
        feed(
            &mut hash,
            relative
                .to_str()
                .expect("source path")
                .replace('\\', "/")
                .as_bytes(),
        );
        feed(&mut hash, &fs::read(&input).expect("source input"));
    }
    println!("cargo:rustc-env=CARLOS_SOURCE_ID={hash:032x}");
}

fn collect(directory: &Path, inputs: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory).expect("source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect(&path, inputs);
        } else if path.is_file() {
            inputs.push(path);
        }
    }
}

fn feed(hash: &mut u128, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        *hash = (*hash ^ u128::from(*byte)).wrapping_mul(PRIME);
    }
}
