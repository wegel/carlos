//! Keep cheap commands literal and recognize substantial code without evaluation.

use crate::summaries::{command_key, eligibility};

#[test]
fn short_commands_and_quoted_operators_stay_literal() {
    for command in [
        "cp source target",
        "git add .",
        "cargo fmt && cargo test",
        "python3 -c 'print(1)'",
    ] {
        assert!(!eligibility::eligible(command), "{command}");
    }
    assert!(!eligibility::eligible(&format!(
        "cp '{}' target",
        "quoted && text; | ".repeat(25)
    )));
    assert!(!eligibility::eligible(&format!(
        "printf '{}'",
        "long data ".repeat(40)
    )));
}

#[test]
fn long_chains_and_embedded_scripts_are_selected() {
    let chain = format!("git add {} && git commit -m 'update'", "path/".repeat(50));
    assert!(eligibility::eligible(&chain));
    let script = format!("python3 - <<'PY'\n{}PY", "print('data')\n".repeat(10));
    assert!(eligibility::eligible(&script));
    assert!(!eligibility::file_based(&script));
    let inline = format!("python3 -c '{}'", "print(1);".repeat(30));
    assert!(eligibility::eligible(&inline));
    assert!(!eligibility::file_based(&inline));
}

#[test]
fn multiline_commands_do_not_require_a_minimum_size() {
    let command = "python3 - <<'PY'\nfrom pathlib import Path\nimport json\np=next(Path('out/ep130-furious-finish/final268-screen').glob('*.jsonl'))\nfor line in p.read_text().splitlines():\n s=json.loads(line);print({k:s.get(k) for k in ['state','position_ms','last_command_id']},s.get('cut_playback',{}).get('position_ms'))\nPY";
    assert!(eligibility::eligible(command));
    assert!(!eligibility::file_based(command));
    for command in [
        "python3 - <<'PY'\nprint(1)\nPY",
        "cp source target\ncp second third",
        "curl \\\n https://example.com",
        "python3 - <<'PY'\n# unmatched quote: '\nprint(1)\nPY",
    ] {
        assert!(eligibility::eligible(command), "{command}");
    }
    assert!(!eligibility::eligible("\ncp source target\n"));
    assert!(!eligibility::file_based("python3 - <<'PY'\nprocess.py\nPY"));
}

#[test]
fn named_scripts_are_selected_but_copied_script_names_are_not() {
    for command in [
        "python3 process.py",
        "python3 -u scripts/process.py",
        "./scripts/fix.sh",
    ] {
        assert!(eligibility::eligible(command));
        assert!(eligibility::file_based(command));
    }
    assert!(!eligibility::eligible("cp process.py backup.py"));
}

#[test]
fn exact_command_bytes_define_identity() {
    assert_eq!(
        command_key("python3 process.py"),
        command_key("python3 process.py")
    );
    assert_ne!(
        command_key("python3 process.py"),
        command_key("python3  process.py")
    );
}
