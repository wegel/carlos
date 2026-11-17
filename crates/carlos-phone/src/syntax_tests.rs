//! Preserve exact source while grammar state distinguishes comments, strings and language tokens.

use crate::{file_lines, highlight_code, PhoneDocument, PhoneSyntaxRole};

#[test]
fn rust_grammar_preserves_unicode_and_multiline_string_state() {
    let source =
        "// hello 🌍\r\nfn main() {\n let word = r#\"first\n second 🦀\"#;\n let count = 42;\n}\n";
    let spans = highlight_code(source.into(), Some("rust".into()), None);
    assert_eq!(
        spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>(),
        source
    );
    assert!(spans
        .iter()
        .any(|span| span.role == PhoneSyntaxRole::Comment && span.text.contains("🌍")));
    assert!(spans
        .iter()
        .any(|span| span.role == PhoneSyntaxRole::String && span.text.contains("second 🦀")));
    assert!(spans
        .iter()
        .any(|span| span.role == PhoneSyntaxRole::Number && span.text.contains("42")));
    assert!(spans
        .iter()
        .any(|span| span.role == PhoneSyntaxRole::Keyword && span.text.contains("fn")));
}

#[test]
fn unsupported_or_costly_input_keeps_all_plain_source_without_opening_remote_paths() {
    for (source, language, path) in [
        ("unknown 🦀\n".into(), Some("not-a-language".into()), None),
        ("x".repeat(300_000), Some("rust".into()), None),
        (
            "x".repeat(9000),
            None,
            Some("/remote/never-open-this.rs".into()),
        ),
    ] {
        let spans = highlight_code(source.clone(), language, path);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].role, PhoneSyntaxRole::Plain);
        assert_eq!(spans[0].text, source);
    }
}

#[test]
fn visible_markdown_code_uses_its_explicit_language_without_changing_the_document() {
    let source = "A paragraph.\n\n```rs\nlet count = 42;\n```\n";
    let document = PhoneDocument::new(source.into()).expect("Markdown");
    let code = document.code(1).expect("visible code");
    assert!(code.iter().any(|span| span.role == PhoneSyntaxRole::Number));
    assert_eq!(
        document.block(1).expect("original block").spans[0].text,
        "let count = 42;\n"
    );
    assert!(document.code(0).is_err());
}

#[test]
fn file_gutters_keep_actual_numbers_and_multiline_code_without_inventing_an_offset() {
    let source = " 40→/* first\n 41→second 🌍 */\n 42→let count = 42;\n";
    let lines = file_lines(source.into(), "/remote/source.rs".into());
    assert_eq!(
        lines
            .iter()
            .filter_map(|line| line.number.as_deref())
            .collect::<Vec<_>>(),
        ["40", "41", "42"]
    );
    let restored: String = lines
        .iter()
        .map(|line| {
            format!(
                "{}{}",
                line.prefix,
                line.spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>()
            )
        })
        .collect();
    assert_eq!(restored, source);
    assert!(lines[1]
        .spans
        .iter()
        .any(|span| span.role == PhoneSyntaxRole::Comment && span.text.contains("🌍")));
    let unnumbered = file_lines("let value = 42;\n".into(), "/remote/source.rs".into());
    assert!(unnumbered.iter().all(|line| line.number.is_none()));
}
