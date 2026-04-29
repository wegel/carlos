//! Dictation vocabulary loading for Whisper initial prompts.
#![allow(dead_code)]

// --- Imports ---

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

// --- Constants ---

pub(crate) const DEFAULT_MAX_PROMPT_CHARS: usize = 512;

const TECH_TERMS: &[&str] = &[
    "claude",
    "codex",
    "carlos",
    "ralph",
    "execplan",
    "refactor",
    "async",
    "await",
    "regex",
    "TypeScript",
    "Rust",
    "npm",
    "git",
    "commit",
    "struct",
    "enum",
    "trait",
];

// --- Public API ---

pub(crate) fn vocabulary_prompt(
    language: &str,
    path: Option<&Path>,
    max_chars: usize,
) -> Result<String> {
    let terms = match path {
        Some(path) if path.is_file() => terms_from_file(path)?,
        _ => default_terms(language),
    };
    Ok(join_limited_terms(&terms, max_chars))
}

// --- Loading ---

fn terms_from_file(path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read dictation vocabulary {}", path.display()))?;
    let terms: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect();
    if terms.is_empty() {
        return Ok(default_terms("en"));
    }
    Ok(terms)
}

fn default_terms(_language: &str) -> Vec<String> {
    TECH_TERMS.iter().map(|term| (*term).to_string()).collect()
}

fn join_limited_terms(terms: &[String], max_chars: usize) -> String {
    let mut out = String::new();
    for term in terms {
        let extra = if out.is_empty() {
            term.len()
        } else {
            term.len() + 2
        };
        if !out.is_empty() && out.len() + extra > max_chars {
            break;
        }
        if out.is_empty() && term.len() > max_chars {
            return truncate_at_char_boundary(term, max_chars);
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(term);
    }
    out
}

fn truncate_at_char_boundary(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

// --- Tests ---

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
