//! Reject bad or cancelled launches before contacting even an isolated native endpoint.

use carlos_proto::session::Backend;

use super::{launch, LaunchError, LaunchOptions, LaunchRequest, RestoreSession};
use crate::launch_cancel::LaunchCancellation;

fn request(root: &std::path::Path) -> LaunchRequest {
    LaunchRequest {
        session: "11111111111111111111111111111111"
            .parse()
            .expect("logical session"),
        backend: Backend::Codex,
        cwd: root.to_owned(),
        name: None,
        restore: None,
        options: LaunchOptions {
            codex_socket: Some(root.join("nonexistent-native.sock")),
            ..LaunchOptions::default()
        },
    }
}

#[test]
fn cancelled_launch_does_not_contact_the_native_backend() {
    let root = tempfile::tempdir().expect("isolated native path");
    let cancel = LaunchCancellation::default();
    cancel.cancel();
    assert!(matches!(
        launch(request(root.path()), &cancel),
        Err(LaunchError::Cancelled)
    ));
}

#[test]
fn missing_saved_native_id_cannot_become_a_new_session() {
    let root = tempfile::tempdir().expect("isolated native path");
    let mut request = request(root.path());
    request.restore = Some(RestoreSession {
        native_thread: String::new(),
        ralph_was_on: true,
    });
    assert!(matches!(
        launch(request, &LaunchCancellation::default()),
        Err(LaunchError::Invalid(_))
    ));
}

#[test]
fn relative_cwd_does_not_depend_on_the_daemons_working_directory() {
    let request = request(std::path::Path::new("relative"));
    assert!(matches!(
        launch(request, &LaunchCancellation::default()),
        Err(LaunchError::Invalid(_))
    ));
}

#[test]
fn explicit_summary_environment_values_keep_the_legacy_trim_and_auto_fallback() {
    for (value, expected) in [
        ("auto", "auto"),
        (" concise ", "concise"),
        ("detailed", "detailed"),
        (" none ", "none"),
        ("", "auto"),
        ("UNKNOWN", "auto"),
        ("NONE", "auto"),
    ] {
        assert_eq!(super::parse_summary_override(value), expected);
    }
}
