//! Prove ownership checks and socket cleanup with actual private Unix endpoints.

use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};

use socket2::{Domain, SockAddr, Socket, Type};

use super::{verify_peer, BoundSocket};

fn mode(path: &std::path::Path, value: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(value))
        .expect("fixture permissions");
}

fn private_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary endpoint directory");
    mode(root.path(), 0o700);
    root
}

#[test]
fn a_second_daemon_cannot_unlink_a_live_endpoint_or_share_its_catalogue() {
    let root = private_root();
    let path = root.path().join("socket");
    let state = root.path().join("state");
    let first = BoundSocket::bind(&path, &state).expect("first daemon owns endpoint");
    assert!(BoundSocket::bind(&path, &root.path().join("other-state")).is_err());
    assert!(BoundSocket::bind(&root.path().join("other-socket"), &state).is_err());
    let connection = UnixStream::connect(first.path()).expect("first listener remains live");
    let accepted = first.listener().accept().expect("accept same-user peer").0;
    verify_peer(&accepted).expect("kernel credentials match");
    drop(connection);
}

#[test]
fn a_proven_stale_owned_socket_can_be_replaced_without_touching_a_regular_file() {
    let root = private_root();
    let path = root.path().join("socket");
    let old = UnixListener::bind(&path).expect("stale fixture");
    mode(&path, 0o600);
    drop(old);
    wait_for_stale(&path);
    let current =
        BoundSocket::bind(&path, &root.path().join("state")).expect("replace only stale socket");
    UnixStream::connect(current.path()).expect("new socket is live");
    drop(current);
    std::fs::write(&path, b"keep this file").expect("non-socket fixture");
    assert!(BoundSocket::bind(&path, &root.path().join("state")).is_err());
    assert_eq!(
        std::fs::read(&path).expect("file preserved"),
        b"keep this file"
    );
}

fn wait_for_stale(path: &std::path::Path) {
    // Parallel process tests can briefly inherit this descriptor between fork and exec.
    // Wait for the kernel's stale-socket result before testing the replacement policy.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let probe = Socket::new(Domain::UNIX, Type::STREAM, None).expect("fixture probe");
        probe.set_nonblocking(true).expect("bounded fixture probe");
        if probe
            .connect(&SockAddr::unix(path).expect("fixture address"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused)
        {
            return;
        }
        assert!(Instant::now() < deadline, "fixture listener is still live");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn socket_symlinks_and_accessible_directories_are_rejected() {
    let root = private_root();
    let target = root.path().join("target");
    std::fs::write(&target, "private data").expect("target");
    let link = root.path().join("socket");
    symlink(&target, &link).expect("socket symlink");
    assert!(BoundSocket::bind(&link, &root.path().join("state")).is_err());
    assert_eq!(
        std::fs::read_to_string(&target).expect("target preserved"),
        "private data"
    );
    let accessible = root.path().join("accessible");
    std::fs::create_dir(&accessible).expect("directory");
    mode(&accessible, 0o777);
    assert!(BoundSocket::bind(&accessible.join("socket"), &root.path().join("state")).is_err());
}

#[test]
fn cleanup_does_not_remove_a_replacement_listeners_path() {
    let root = private_root();
    let path = root.path().join("socket");
    let first = BoundSocket::bind(&path, &root.path().join("state")).expect("first");
    std::fs::remove_file(&path).expect("replace only this test endpoint");
    let replacement = UnixListener::bind(&path).expect("another listener owns pathname");
    mode(&path, 0o600);
    drop(first);
    UnixStream::connect(&path).expect("replacement path survives old guard cleanup");
    drop(replacement);
}

#[test]
fn a_live_listener_without_our_lock_is_still_never_unlinked() {
    let root = private_root();
    let path = root.path().join("socket");
    let other = UnixListener::bind(&path).expect("independent listener");
    mode(&path, 0o600);
    assert!(BoundSocket::bind(&path, &root.path().join("state")).is_err());
    UnixStream::connect(&path).expect("independent listener survives");
    drop(other);
}

#[test]
fn kernel_credentials_reject_a_foreign_uid_even_if_fixture_permissions_are_relaxed() {
    if unsafe { libc::geteuid() } != 0 {
        return;
    } // Only this isolated child needs setuid privileges.
    use std::os::unix::process::CommandExt;
    let root = private_root();
    let path = root.path().join("socket");
    let bound = BoundSocket::bind(&path, &root.path().join("state")).expect("owned endpoint");
    mode(root.path(), 0o711);
    mode(&path, 0o666);
    let mut peer = std::process::Command::new("python3")
        .args([
            "-c",
            "import socket,sys;s=socket.socket(socket.AF_UNIX);s.connect(sys.argv[1]);s.close()",
            path.to_str().expect("socket path"),
        ])
        .uid(65534)
        .gid(65534)
        .spawn()
        .expect("isolated other-user peer");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let accepted = loop {
        match bound.listener().accept() {
            Ok((stream, _)) => break stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
            Err(error) => panic!("fixture peer did not connect: {error}"),
        }
    };
    assert_eq!(
        verify_peer(&accepted)
            .expect_err("kernel uid must be checked")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(peer.wait().expect("peer stops").success());
}
