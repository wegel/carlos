// CLI handshake and session-history-file replay tests for the Claude backend.

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
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_BACKEND");
    std::env::set_var("CARLOS_BACKEND", "claude");

    let parsed = parse_cli_args(Vec::<String>::new()).expect("parse");
    assert_eq!(parsed.backend, Backend::Claude);

    match old {
        Some(value) => std::env::set_var("CARLOS_BACKEND", value),
        None => std::env::remove_var("CARLOS_BACKEND"),
    }
}
