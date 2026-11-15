//! Keep backend flags and environment defaults independent of native session fixtures.

use super::{parse_cli_args, Backend};

#[test]
fn parse_cli_args_supports_backend_flag() {
    let parsed = parse_cli_args(vec![
        "--backend".to_string(),
        "claude".to_string(),
        "continue".to_string(),
    ])
    .expect("parse");
    assert_eq!(parsed.backend, Backend::Claude);
    assert!(parsed.mode_continue);
}

#[test]
fn parse_cli_args_uses_backend_env_default() {
    const CHILD: &str = "CARLOS_CLI_DEFAULT_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "app::cli::tests::parse_cli_args_uses_backend_env_default",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("CARLOS_BACKEND", "claude")
            .output()
            .expect("isolated environment case");
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let parsed = parse_cli_args(Vec::<String>::new()).expect("parse");
    assert_eq!(parsed.backend, Backend::Claude);
}

#[test]
fn resume_hint_formats_the_logical_daemon_resume_command() {
    let session = "a".repeat(32).parse().expect("logical session");
    assert_eq!(
        super::daemon_resume_hint(std::path::Path::new("/tmp/carlos.sock"), &session),
        format!(
            "to resume this session use:\ncarlos --daemon-socket /tmp/carlos.sock resume {session}"
        )
    );
}

#[test]
fn styled_resume_hint_colors_only_the_logical_command_line() {
    let session = "a".repeat(32).parse().expect("logical session");
    assert_eq!(super::styled_daemon_resume_hint(std::path::Path::new("/tmp/carlos.sock"), &session),
        format!("to resume this session use:\n\x1b[94mcarlos --daemon-socket /tmp/carlos.sock resume {session}\x1b[0m"));
}
