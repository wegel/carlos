use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::{vocabulary_prompt, DEFAULT_MAX_PROMPT_CHARS};

fn temp_root() -> PathBuf {
    let path = std::env::temp_dir().join(format!("carlos-vocab-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("temp root");
    path
}

#[test]
fn missing_vocabulary_uses_default_terms() {
    let root = temp_root();
    let prompt = vocabulary_prompt(
        "fr",
        Some(&root.join("missing.txt")),
        DEFAULT_MAX_PROMPT_CHARS,
    )
    .expect("prompt");

    assert!(prompt.contains("TypeScript"));
    assert!(prompt.contains("Rust"));
}

#[test]
fn empty_vocabulary_uses_default_terms() {
    let root = temp_root();
    let path = write_vocab(&root, "");

    let prompt = vocabulary_prompt("en", Some(&path), 512).expect("prompt");

    assert!(prompt.contains("codex"));
    assert!(prompt.contains("trait"));
}

#[test]
fn vocabulary_ignores_comments_and_blank_lines() {
    let root = temp_root();
    let path = write_vocab(&root, "# comment\n\nserde\nTypeScript\n");

    let prompt = vocabulary_prompt("en", Some(&path), 512).expect("prompt");

    assert_eq!(prompt, "serde, TypeScript");
}

#[test]
fn vocabulary_truncates_from_the_end() {
    let root = temp_root();
    let path = write_vocab(&root, "alpha\nbeta\ngamma\n");

    let prompt = vocabulary_prompt("en", Some(&path), 11).expect("prompt");

    assert_eq!(prompt, "alpha, beta");
}

fn write_vocab(root: &Path, text: &str) -> PathBuf {
    let path = root.join("vocab.txt");
    fs::write(&path, text).expect("write vocab");
    path
}
