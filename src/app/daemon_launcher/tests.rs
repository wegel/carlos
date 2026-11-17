//! Check managed routing, private host paths and exact compatibility choices without user files.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::paths::Paths;
use crate::app::cli::parse_cli_args;

#[test]
fn a_starting_daemon_can_finish_socket_permissions_before_its_first_reply() {
    let root = tempfile::tempdir().expect("bind/chmod fixture");
    let path = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&path).expect("bound listener");
    listener
        .set_nonblocking(true)
        .expect("bounded fixture accept");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777))
        .expect("bind has not finished chmod");
    let server_path = path.clone();
    let server = std::thread::spawn(move || greet_private_socket(listener, server_path));
    let connected = super::probe::connect(&path, None, Instant::now() + Duration::from_secs(2));
    server
        .join()
        .expect("fixture worker")
        .expect("fixture greeting");
    assert!(connected.expect("greet after private setup").is_some());
}

fn greet_private_socket(listener: UnixListener, path: PathBuf) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                anyhow::ensure!(Instant::now() < deadline, "launcher never connected");
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(error.into()),
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut request = String::new();
    BufReader::new(&mut stream).read_line(&mut request)?;
    let request: serde_json::Value = serde_json::from_str(&request)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({
            "jsonrpc":"2.0", "id":request["id"], "result":fixture_hello()
        }),
    )?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn fixture_hello() -> carlos_proto::commands::HelloResult {
    carlos_proto::commands::HelloResult {
        identity: Some(carlos_proto::identity::DaemonIdentity {
            build: crate::build_info(),
            process: std::process::id(),
            codex_socket: None,
        }),
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "local".into(),
        daemon_version: env!("CARGO_PKG_VERSION").into(),
        instance: "a".repeat(32).parse().expect("instance"),
        client: "b".repeat(32).parse().expect("client"),
        resumed: false,
        next_operation: carlos_proto::ids::OperationId::new(1),
        resume_token: "c".repeat(64).parse().expect("private fixture credential"),
    }
}

#[test]
fn a_different_source_build_is_compatible_unless_exact_matching_was_requested() {
    let mut hello = fixture_hello();
    hello.identity.as_mut().expect("identity").build.source = "0".repeat(32);
    super::probe::validate(&hello, None, false).expect("same wire protocol");
    assert!(super::probe::validate(&hello, None, true).is_err());
}

#[test]
fn incompatible_and_inconsistent_protocol_versions_are_always_rejected() {
    let mut hello = fixture_hello();
    hello.protocol += 1;
    assert!(super::probe::validate(&hello, None, false).is_err());
    hello.protocol = carlos_proto::PROTOCOL_VERSION;
    hello.identity.as_mut().expect("identity").build.protocol += 1;
    assert!(super::probe::validate(&hello, None, false).is_err());
}

#[test]
fn optional_identity_is_required_only_for_explicit_build_or_endpoint_checks() {
    let mut hello = fixture_hello();
    hello.identity = None;
    super::probe::validate(&hello, None, false).expect("compatible legacy greeting");
    assert!(super::probe::validate(&hello, None, true).is_err());
    assert!(super::probe::validate(&hello, Some(std::path::Path::new("/native")), false).is_err());
}

#[test]
fn a_companion_that_never_reports_its_identity_is_killed_before_the_deadline_returns() {
    let root = tempfile::tempdir().expect("stuck companion fixture");
    let daemon = root.path().join("carlosd");
    // A freshly written executable can stay writable in another test's fork until exec.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/app/daemon_launcher/fixtures/stuck-companion.sh");
    symlink(fixture, &daemon).expect("stable companion fixture");
    let started = Instant::now();
    let error = super::companion::check(&daemon, started + Duration::from_millis(80))
        .expect_err("identity request must time out");
    assert!(
        format!("{error:#}").contains("startup deadline"),
        "unexpected companion error: {error:#}"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[cfg(target_os = "linux")]
#[test]
fn a_full_unix_accept_queue_cannot_block_the_launcher_past_its_deadline() {
    use socket2::{Domain, SockAddr, Socket, Type};

    let root = tempfile::tempdir().expect("full socket queue fixture");
    let path = root.path().join("daemon.sock");
    let listener = Socket::new(Domain::UNIX, Type::STREAM, None).expect("listener");
    listener
        .bind(&SockAddr::unix(&path).expect("socket address"))
        .expect("bind fixture");
    listener.listen(1).expect("small accept queue");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("private socket");
    let mut queued = Vec::new();
    let mut saturated = false;
    for _ in 0..128 {
        let connection = Socket::new(Domain::UNIX, Type::STREAM, None).expect("queued socket");
        connection
            .set_nonblocking(true)
            .expect("bounded queue fill");
        match connection.connect(&SockAddr::unix(&path).expect("queued address")) {
            Ok(()) => queued.push(connection),
            Err(error) if error.raw_os_error() == Some(libc::EAGAIN) => {
                saturated = true;
                break;
            }
            Err(error) => panic!("fill accept queue: {error}"),
        }
    }
    assert!(saturated, "fixture fills the finite accept queue");
    let started = Instant::now();
    let error = super::probe::connect(&path, None, started + Duration::from_millis(80))
        .err()
        .expect("full listener must time out");
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .expect("connect error")
            .kind(),
        std::io::ErrorKind::TimedOut
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

fn options(arguments: &[&str]) -> anyhow::Result<crate::app::cli::CliOptions> {
    parse_cli_args(arguments.iter().map(|value| (*value).to_owned()))
}

#[test]
fn managed_route_remains_explicit_and_rejects_other_routes() {
    assert!(!options(&[]).expect("embedded default remains").daemon);
    assert!(
        options(&["--daemon", "continue"])
            .expect("managed continue")
            .daemon
    );
    assert!(options(&["--daemon", "--daemon-socket", "/tmp/attach"]).is_err());
    assert!(options(&["--daemon", "perf-session", "--synthetic"]).is_err());
    assert!(
        !options(&["--daemon-socket", "/tmp/attach"])
            .expect("explicit attach")
            .daemon
    );
}

#[test]
fn private_runtime_and_state_paths_do_not_depend_on_session_cwd() {
    let root = tempfile::tempdir().expect("private host fixture");
    let paths = Paths::resolve(
        Some(root.path().join("runtime").into_os_string()),
        Some(root.path().join("state").into_os_string()),
        None,
    )
    .expect("private host paths");
    assert_eq!(paths.socket, root.path().join("runtime/carlos/daemon.sock"));
    assert_eq!(paths.state, root.path().join("state/carlos/daemon"));
    for path in [
        paths.socket.parent().expect("socket parent"),
        paths.state.as_path(),
    ] {
        assert_eq!(
            path.metadata()
                .expect("private directory")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    let first = paths
        .lock(Instant::now() + Duration::from_millis(100))
        .expect("first launcher lock");
    assert!(paths
        .lock(Instant::now() + Duration::from_millis(20))
        .is_err());
    drop(first);
    paths
        .lock(Instant::now() + Duration::from_millis(100))
        .expect("released launcher lock");
}

#[test]
fn managed_paths_reject_symlinks_and_public_directories() {
    let root = tempfile::tempdir().expect("unsafe path fixture");
    let state = Some(root.path().join("state").into_os_string());
    let runtime = root.path().join("runtime/carlos");
    std::fs::create_dir_all(&runtime).expect("runtime fixture");
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755))
        .expect("public mode");
    assert!(Paths::resolve(
        Some(root.path().join("runtime").into_os_string()),
        state.clone(),
        None
    )
    .is_err());
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700))
        .expect("private mode");
    std::fs::rename(&runtime, runtime.with_extension("target")).expect("move directory");
    symlink(runtime.with_extension("target"), &runtime).expect("linked runtime leaf");
    assert!(Paths::resolve(
        Some(root.path().join("runtime").into_os_string()),
        state,
        None
    )
    .is_err());
}
