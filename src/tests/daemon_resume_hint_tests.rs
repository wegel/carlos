//! Keep explicit native endpoint rules and quote logical daemon resume addresses.

#[test]
fn socket_option_requires_codex_conversations_and_resume_hints_quote_exact_paths() {
    use crate::app::cli::{daemon_resume_hint, parse_cli_args};

    use std::path::Path;
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    };
    let parsed = parse_cli_args(args(&[
        "--backend",
        "codex",
        "--codex-socket",
        "socket with spaces",
        "resume",
        "root",
    ]))
    .expect("explicit socket");
    assert!(parsed.codex_socket.expect("socket").is_absolute());
    assert!(parse_cli_args(args(&["--codex-socket", "/tmp/rpc", "--backend", "claude"])).is_err());
    assert!(parse_cli_args(args(&[
        "perf-session",
        "--synthetic",
        "--codex-socket",
        "/tmp/rpc"
    ]))
    .is_err());
    let session: carlos_proto::ids::SessionId = "a".repeat(32).parse().expect("logical session");
    let hint = daemon_resume_hint(Path::new("/tmp/a' socket"), &session);
    let words = shlex::split(hint.lines().nth(1).expect("command")).expect("shell-safe hint");
    assert_eq!(
        words,
        [
            "carlos",
            "--daemon-socket",
            "/tmp/a' socket",
            "resume",
            session.as_str()
        ]
    );
}
