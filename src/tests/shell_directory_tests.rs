//! Prove literal directory guards without changing scripts or accepting ambiguous shell syntax.

use crate::app::shell_directory::directory_prefix;

#[test]
fn literal_quoted_escaped_unicode_and_adjacent_operator_prefixes_keep_the_exact_body() {
    for (raw, directory) in [
        ("cd /work/project&&rtk cargo test", "/work/project"),
        (
            "cd -- '/work/café 日本語' && rtk cargo test",
            "/work/café 日本語",
        ),
        (
            "cd /work/escaped\\ space && rtk cargo test",
            "/work/escaped space",
        ),
        ("cd '/work/it'\\''s' && rtk cargo test", "/work/it's"),
        ("cd '/work/$literal' && rtk cargo test", "/work/$literal"),
        (
            "cd \"/work/\\$literal\" && rtk cargo test",
            "/work/$literal",
        ),
        ("cd -- -literal && rtk cargo test", "-literal"),
    ] {
        let prefix = directory_prefix(raw).expect(raw);
        assert_eq!(prefix.directory, directory);
        assert_eq!(prefix.body, "rtk cargo test");
    }
}

#[test]
fn multiline_heredocs_and_later_directory_changes_remain_byte_identical() {
    let body = "python3 - <<'PY'\nprint('cd /kept && literal')\nprint(\"don't rewrite it\")\nPY\ncd ../elsewhere && printf done\n";
    let command = format!("cd \\\n  '/work/project' &&\n{body}");
    assert_eq!(
        directory_prefix(&command).expect("literal prefix").body,
        body
    );
}

#[test]
fn body_keeps_non_shell_whitespace_and_carriage_returns() {
    for body in ["\u{a0}program", "\rprogram", "program\r\nbody\u{a0}"] {
        let command = format!("cd /work && {body}");
        assert_eq!(directory_prefix(&command).expect("prefix").body, body);
    }
    assert_eq!(
        directory_prefix("cd /work/\u{a0}name&&program")
            .expect("literal NBSP")
            .directory,
        "/work/\u{a0}name"
    );
}

#[test]
fn expansions_assignments_subshells_extra_operands_and_other_control_operators_stay_unchanged() {
    for raw in [
        "cd $HOME && command",
        "cd \"$HOME\" && command",
        "cd $(pwd) && command",
        "cd `pwd` && command",
        "cd ~/project && command",
        "cd /work/* && command",
        "cd /work/{one,two} && command",
        "cd - && command",
        "cd -P /work && command",
        "cd && command",
        "cd -- && command",
        "cd /work extra && command",
        "cd /work || command",
        "cd /work; command",
        "cd /work & command",
        "cd /work\ncommand",
        "cd /work\n&& command",
        "cd\n/work && command",
        "(cd /work && command)",
        "PWD=/work cd /work && command",
        "env cd /work && command",
        "rtk proxy cd /work && command",
        "printf 'cd /work && command'",
        "ssh host 'cd /work && command'",
        "cd '/unfinished && command",
        "cd /work &&",
        "cd /work &&& command",
        "cd /work && || command",
        "cd /work\\",
    ] {
        assert!(directory_prefix(raw).is_none(), "{raw}");
    }
}
