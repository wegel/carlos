//! Check RTK and shell wrappers without executing commands from the transcript.

use crate::app::shell_command::{analyze_shell_command, display_shell_command};
use crate::app::shell_read::single_source_path;
use crate::app::tool_shell::command_summary_from_shell_cmd;

#[test]
fn wrappers_and_flags_reveal_the_command_for_labels() {
    for (raw, expected) in [
        ("rtk proxy cat main.rs", "cat main.rs"),
        (
            "rtk --verbose --ultra-compact proxy --skip-env -- cat 'main file.rs'",
            "cat 'main file.rs'",
        ),
        (
            "'/opt/RTK tools/rtk' -vv proxy '/bin/cat' main.rs",
            "'/bin/cat' main.rs",
        ),
        ("rtk proxy rtk proxy cat main.rs", "cat main.rs"),
        ("rtk bypass cat main.rs", "cat main.rs"),
        ("rtk run -c 'cat main.rs'", "cat main.rs"),
        ("rtk run --command='cat main.rs'", "cat main.rs"),
        ("rtk run 'cat main.rs'", "cat main.rs"),
        (
            "rtk run --skip-env --command 'rtk proxy cat main.rs'",
            "cat main.rs",
        ),
        (
            "rtk proxy bash -o pipefail -c 'rtk proxy cat main.rs'",
            "cat main.rs",
        ),
        (
            "TEST_FLAG=yes env -i -u UNUSED_FLAG rtk proxy cat main.rs",
            "cat main.rs",
        ),
        (
            "env TEST_FLAG='two words' command rtk proxy cat main.rs",
            "cat main.rs",
        ),
    ] {
        let command = analyze_shell_command(raw);
        assert_eq!(command.text, expected, "{raw}");
        assert!(command.has_rtk, "{raw}");
        assert!(!command.rtk_native, "{raw}");
    }
}

#[test]
fn native_commands_remain_distinct_from_proxy_commands() {
    let native = analyze_shell_command("rtk -vv read -n main.rs");
    assert!(native.rtk_native);
    assert_eq!(native.text, "read -n main.rs");
    let proxied = analyze_shell_command("rtk proxy read -n main.rs");
    assert!(
        !proxied.rtk_native,
        "the shell's read builtin is not RTK's file reader"
    );
    assert!(single_source_path("rtk proxy read -n main.rs").is_none());
}

#[test]
fn displayed_commands_keep_rtk_flags_quotes_and_script_bodies() {
    for command in [
        "rtk proxy cat 'main file.rs'",
        "rtk --verbose proxy --skip-env -- cat main.rs | sed -n '1,5p'",
        "rtk run -c 'git status && git diff'",
        "python3 - <<'PY'\nprint(\"example -lc 'hello'\")\nPY",
        "printf '%s' 'rtk proxy cat example.rs'",
    ] {
        assert_eq!(display_shell_command(command), command);
    }
    assert_eq!(
        display_shell_command("/bin/zsh -lc 'rtk proxy cat main.rs'"),
        "rtk proxy cat main.rs"
    );
}

#[test]
fn unknown_flags_help_and_incomplete_wrappers_do_not_claim_a_file_read() {
    for command in [
        "rtk",
        "rtk proxy",
        "rtk proxy --help",
        "rtk --unknown cat main.rs",
        "env --unknown rtk proxy cat main.rs",
        "rtk read --help main.rs",
        "rtk read --unknown main.rs",
        "rtk proxy cat 'unfinished",
        "printf '%s' 'rtk proxy cat main.rs'",
    ] {
        assert!(single_source_path(command).is_none(), "{command}");
    }
}

#[test]
fn read_flags_and_quoted_paths_do_not_turn_option_values_into_filenames() {
    for (command, expected) in [
        ("rtk read -n 'src/main file.rs'", "src/main file.rs"),
        (
            "rtk --ultra-compact read -l aggressive -m 20 main.rs",
            "main.rs",
        ),
        (
            "rtk read --level=none --max-lines=20 main.rs --tail-lines 5",
            "main.rs",
        ),
        ("rtk read -m20 -lminimal main.rs", "main.rs"),
        ("rtk proxy head -n 20 main.rs", "main.rs"),
        ("rtk proxy tail --lines=10 main.rs", "main.rs"),
        ("rtk proxy nl -b a -s ': ' main.rs", "main.rs"),
        ("rtk proxy cat -- '-file.rs'", "-file.rs"),
        (
            "rtk run -c 'nl -ba main.rs | rtk proxy sed -n \"1,5p\"'",
            "main.rs",
        ),
        ("rtk proxy sed -n '1,20p' main.rs", "main.rs"),
        ("rtk run \"cat 'main file.rs'\"", "main file.rs"),
    ] {
        assert_eq!(
            single_source_path(command).as_deref(),
            Some(expected),
            "{command}"
        );
    }
}

#[test]
fn transformed_summaries_multiple_files_and_compound_commands_do_not_claim_one_source() {
    for command in [
        "rtk read a.rs b.py",
        "rtk proxy cat a.rs b.py",
        "rtk smart main.rs",
        "rtk err cat main.rs",
        "rtk summary cat main.rs",
        "rtk proxy cat main.rs && cargo test",
        "rtk proxy cat main.rs; cat other.py",
        "rtk proxy cat main.rs | wc -l",
        "rtk proxy cat main.rs | sed -n '1,5p' | wc -l",
        "rtk proxy cat main.rs |",
        "rtk proxy cat main.rs > output.rs",
        "rtk proxy cat *.rs",
        "rtk run cat 'a file.rs'",
        "rtk proxy cat \"$SOURCE_FILE\"",
        "rtk proxy sed -n 's/old/new/p' main.rs",
    ] {
        assert!(single_source_path(command).is_none(), "{command}");
    }
}

#[test]
fn rtk_commands_get_read_search_build_test_and_meta_labels() {
    for (command, expected) in [
        ("rtk read -n /repo/main.rs", "→ Read main.rs"),
        ("rtk proxy cat 'a file.rs' b.py", "→ Read a file.rs, b.py"),
        ("rtk smart main.rs", "→ Inspect main.rs"),
        (
            "rtk proxy grep -E -r 'a|b' src",
            "✱ Search src [pattern=a|b]",
        ),
        ("rtk rg -eneedle src", "✱ Search src [pattern=needle]"),
        (
            "rtk --verbose proxy rg -n -g '*.rs' -e 'a|b' src",
            "✱ Search src [pattern=a|b]",
        ),
        (
            "rtk rg --regexp=needle --glob=*.rs src",
            "✱ Search src [pattern=needle]",
        ),
        (
            "rtk proxy grep -n -A 3 'needle' src",
            "✱ Search src [pattern=needle]",
        ),
        ("rtk --ultra-compact git --no-pager -C /repo diff", "Δ Diff"),
        ("rtk git status --short", "Git status --short"),
        ("rtk cargo test -p example", "Test cargo test -p example"),
        (
            "rtk proxy cargo build --release",
            "Build cargo build --release",
        ),
        ("rtk cargo clippy", "Check cargo clippy"),
        ("rtk test rtk proxy cargo test", "Test cargo test"),
        ("rtk err cargo build", "Run cargo build (errors only)"),
        ("rtk ls src", "→ List src"),
        ("rtk gain --history", "RTK gain --history"),
        ("rtk config", "RTK config"),
        ("rtk proxy python3 script.py", "run python3 script.py"),
    ] {
        assert_eq!(
            command_summary_from_shell_cmd(command, Some("/repo")).as_deref(),
            Some(expected),
            "{command}"
        );
    }
}
