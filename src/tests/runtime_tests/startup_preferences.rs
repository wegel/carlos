// Check startup choices independently of whether the session has a backend choice yet.

#[test]
fn startup_preferences_keep_effective_choices_for_each_backend_and_launch_mode() {
    if run_isolated_startup_test() {
        return;
    }
    let directory = tempfile::tempdir().expect("preferences directory");
    let path = directory.path().join("runtime-defaults.json");
    for (contents, saved) in [
        (None, false),
        (Some("not JSON"), false),
        (Some(r#"{"model":7,"effort":false,"summary":[]}"#), false),
        (Some(r#"{"model":" ","effort":"","summary":" "}"#), false),
        (
            Some(r#"{"model":" saved ","effort":" low ","summary":" detailed "}"#),
            true,
        ),
    ] {
        if let Some(contents) = contents {
            std::fs::write(&path, contents).expect("preference fixture");
        }
        let preferences = load_runtime_defaults_from(&path);
        for backend in [Backend::Codex, Backend::Claude, Backend::Grok] {
            for mode in ["new", "resume", "continue"] {
                check_startup_choices(backend, mode, preferences.clone(), saved);
            }
        }
    }
}

fn check_startup_choices(backend: Backend, mode: &str, preferences: RuntimeDefaults, saved: bool) {
    let mut app = AppState::with_runtime_defaults("startup".into(), preferences);
    assert_eq!(app.next_turn_runtime_settings(), (None, None, None));
    if backend == Backend::Codex && mode == "new" && saved {
        check_new_codex_saved_choices(&mut app);
        return;
    }
    let preferences = app.session.runtime_settings().startup_preferences();
    let (native, fallback, expected) = match (backend, mode == "new", saved) {
        (Backend::Codex, _, _) => (
            ThreadRuntimeSettings {
                model: Some("native".into()),
                effort: Some("high".into()),
                summary: Some("native-summary".into()),
            },
            codex_runtime_defaults(None),
            [Some("native"), Some("high"), Some("native-summary")],
        ),
        (Backend::Claude, _, true) => (
            ThreadRuntimeSettings::default(),
            claude_runtime_defaults(),
            [Some("saved"), Some("low"), None],
        ),
        (Backend::Claude, _, false) => (
            ThreadRuntimeSettings::default(),
            claude_runtime_defaults(),
            [None; 3],
        ),
        (Backend::Grok, _, true) => (
            ThreadRuntimeSettings::default(),
            crate::app::backend_setup::grok_runtime_defaults(),
            [Some("saved"), Some("low"), None],
        ),
        (Backend::Grok, _, false) => (
            ThreadRuntimeSettings::default(),
            crate::app::backend_setup::grok_runtime_defaults(),
            [Some("grok-4.6"), Some("high"), None],
        ),
    };
    let resolved = resolve_backend_runtime_settings(
        native,
        preferences,
        &fallback,
        &[],
        backend == Backend::Codex,
    );
    app.set_runtime_capabilities(true, backend == Backend::Codex);
    app.set_runtime_settings(resolved.model, resolved.effort, resolved.summary);
    assert_eq!(
        app.next_turn_runtime_settings(),
        settings_values(expected),
        "{backend:?} {mode}"
    );
}

#[test]
fn codex_without_native_choices_uses_saved_choices_or_the_same_fallbacks() {
    for (preferences, expected) in [
        (
            RuntimeDefaults::default(),
            [Some("gpt-5.5"), Some("high"), Some("concise")],
        ),
        (
            RuntimeDefaults {
                model: Some("saved".into()),
                effort: Some("low".into()),
                summary: Some("detailed".into()),
            },
            [Some("saved"), Some("low"), Some("detailed")],
        ),
    ] {
        let mut app = AppState::with_runtime_defaults("startup".into(), preferences.clone());
        let resolved = resolve_backend_runtime_settings(
            ThreadRuntimeSettings::default(),
            app.session.runtime_settings().startup_preferences(),
            &codex_runtime_defaults(None),
            &[],
            true,
        );
        app.set_runtime_settings(resolved.model, resolved.effort, resolved.summary);
        assert_eq!(app.next_turn_runtime_settings(), settings_values(expected));
    }
}

fn check_new_codex_saved_choices(app: &mut AppState) {
    let mut command = std::process::Command::new("python3");
    command.args(["-u", "-c", r#"
import json,sys
for line in sys.stdin:
 a=json.loads(line)
 assert a['method']=='model/list'
 models=[{'id':name,'model':name,'displayName':name,'isDefault':name=='native',
          'supportedReasoningEfforts':[{'reasoningEffort':e,'description':e} for e in ['low','high']]}
         for name in ['native','saved']]
 print(json.dumps({'id':a['id'],'result':{'data':models,'nextCursor':None}}))
"#]);
    let client = crate::protocol::AppServerClient::start_command(command).expect("startup peer");
    crate::app::backend_setup::configure_new_codex_for_test(
        app,
        &client,
        r#"{"result":{"model":"native","reasoningEffort":"high","summary":"native-summary","thread":{"id":"startup","turns":[]}}}"#,
    );
    assert_eq!(
        app.next_turn_runtime_settings(),
        settings_values([Some("saved"), Some("low"), Some("native-summary")]),
    );
    assert_eq!(app.codex.settings.deferred, Some(json!({"model":"saved","effort":"low"})));
}

fn run_isolated_startup_test() -> bool {
    const CHILD: &str = "CARLOS_M5D_STARTUP_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let root = tempfile::tempdir().expect("isolated startup files");
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "app::tests::runtime_tests::startup_preferences_keep_effective_choices_for_each_backend_and_launch_mode", "--nocapture"])
            .env(CHILD, "1")
            .env_remove("CARLOS_REASONING_SUMMARY")
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_STATE_HOME", root.path().join("state"))
            .env("XDG_CACHE_HOME", root.path().join("cache"))
            .output()
            .expect("isolated startup test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        // A renamed test would make the child run nothing and still exit with success.
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        return true;
    }
    false
}
