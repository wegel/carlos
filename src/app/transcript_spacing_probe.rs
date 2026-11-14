//! Measure the old and new one-row query alone with bounded fixed source fixtures.

use std::env;
use std::fs;
use std::time::Instant;

use carlos_core::session::models::Role;
use serde_json::json;

use super::super::single_line_reasoning;
use super::{input, old};

#[test]
#[ignore = "run old/new separately with an explicit bounded source size"]
fn measure_one_row_reasoning() {
    let mode = env::var("CARLOS_ONE_ROW_MODE").expect("old or new");
    let case = env::var("CARLOS_ONE_ROW_CASE").expect("fixture case");
    let size: usize = env::var("CARLOS_ONE_ROW_BYTES")
        .expect("source bytes")
        .parse()
        .expect("integer bytes");
    let width: usize = env::var("CARLOS_ONE_ROW_WIDTH")
        .unwrap_or_else(|_| "28".into())
        .parse()
        .expect("width");
    let text = fixture(&case, size);
    let started = Instant::now();
    let result = match mode.as_str() {
        "old" => old(Role::Reasoning, &text, width),
        "new" => single_line_reasoning(input(Role::Reasoning, &text), width),
        _ => panic!("explicit old or new mode"),
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "ONE_ROW {}",
        json!({"mode":mode,"case":case,"bytes":text.len(),"width":width,
        "result":result,"elapsed_ms":elapsed_ms,"blake3":blake3::hash(text.as_bytes()).to_hex().to_string(),
        "status":status()})
    );
}

fn fixture(case: &str, size: usize) -> String {
    match case {
        "late_unicode" => "x".repeat(size) + "🙂",
        "summary" => "**".to_string() + &"x".repeat(size) + "🙂**",
        "unicode_words" => "界🙂é\t words ".repeat(size / "界🙂é\t words ".len()),
        "overwide" => "क्".repeat(size / "क्".len()) + "क",
        "zero_width" => "\u{200b}".repeat(size / 3),
        _ => panic!("known fixed fixture"),
    }
}

fn status() -> String {
    fs::read_to_string("/proc/self/status")
        .expect("process status")
        .lines()
        .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        .collect::<Vec<_>>()
        .join("; ")
}
