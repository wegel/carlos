//! Preserve old choices while selecting themes independently for each screen size.

use std::fs;

use serde_json::json;

use crate::app::layout_state::{ThemePreferences, TranscriptTheme};
use crate::app::models::TerminalSize;
use crate::app::tool_output_preferences::{load_themes_from, save_appearance_to};
use crate::app::tool_output_state::ToolOutputMode;

#[test]
fn each_screen_size_can_select_either_theme() {
    for desktop in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
        for phone in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
            let themes = ThemePreferences { desktop, phone };
            for (width, height, expected) in [
                (80, 20, desktop),
                (160, 48, desktop),
                (79, 40, phone),
                (100, 19, phone),
            ] {
                assert_eq!(themes.for_size(TerminalSize { width, height }), expected);
            }
        }
    }
}

#[test]
fn legacy_overrides_migrate_and_new_fields_override_them_individually() {
    let root = tempfile::tempdir().expect("settings directory");
    let path = root.path().join("settings.json");
    for (legacy, desktop, phone) in [
        ("auto", TranscriptTheme::Gutter, TranscriptTheme::Flow),
        ("desktop", TranscriptTheme::Gutter, TranscriptTheme::Gutter),
        ("phone", TranscriptTheme::Flow, TranscriptTheme::Flow),
    ] {
        fs::write(&path, json!({"layout":legacy}).to_string()).expect("legacy settings");
        assert_eq!(load_themes_from(&path), ThemePreferences { desktop, phone });
    }
    fs::write(
        &path,
        json!({"layout":"phone","themes":{"desktop":"gutter","phone":"invalid"}}).to_string(),
    )
    .expect("partial preferences");
    assert_eq!(
        load_themes_from(&path),
        ThemePreferences {
            desktop: TranscriptTheme::Gutter,
            phone: TranscriptTheme::Flow
        }
    );
    fs::write(&path, "bad json").expect("invalid settings");
    assert_eq!(load_themes_from(&path), ThemePreferences::default());
}

#[test]
fn saving_themes_removes_legacy_field_and_preserves_other_preferences() {
    let root = tempfile::tempdir().expect("settings directory");
    let path = root.path().join("settings.json");
    fs::write(
        &path,
        json!({"layout":"phone","ai_tool_labels":false,"other":17}).to_string(),
    )
    .expect("old settings");
    let themes = ThemePreferences {
        desktop: TranscriptTheme::Flow,
        phone: TranscriptTheme::Gutter,
    };
    save_appearance_to(&path, ToolOutputMode::Full, themes).expect("save appearance");
    assert_eq!(load_themes_from(&path), themes);
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("settings")).expect("JSON");
    assert_eq!(saved["themes"], json!({"desktop":"flow","phone":"gutter"}));
    assert!(saved.get("layout").is_none());
    assert_eq!(saved["tool_output"], "full");
    assert_eq!(saved["ai_tool_labels"], false);
    assert_eq!(saved["other"], 17);
}
