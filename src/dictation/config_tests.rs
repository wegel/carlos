use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::{
    default_config_path, load_dictation_config, load_dictation_config_from_path, DictationConfig,
    DictationPathEnv, RawDictationConfig,
};

fn temp_root() -> PathBuf {
    let path = std::env::temp_dir().join(format!("carlos-dictation-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("temp root");
    path
}

fn test_env(root: &Path) -> DictationPathEnv {
    DictationPathEnv {
        home: Some(root.join("home")),
        xdg_config_home: Some(root.join("config")),
        xdg_cache_home: Some(root.join("cache")),
    }
}

#[test]
fn missing_config_uses_default_english_profile() {
    let root = temp_root();
    let env = test_env(&root);
    let path = default_config_path(&env).expect("default path");

    let config = load_dictation_config_from_path(&path, None, &env).expect("config");
    let profile = config.profiles.get("en").expect("english profile");

    assert_eq!(config.default_profile, "en");
    assert_eq!(config.active_profile, "en");
    assert_eq!(profile.name, "English");
    assert_eq!(profile.language, "en");
    assert_eq!(profile.model, root.join("cache/carlos/whisper-model.bin"));
}

#[test]
fn malformed_toml_reports_config_parse_error() {
    let root = temp_root();
    let env = test_env(&root);
    let path = root.join("dictation.toml");
    fs::write(&path, "default_profile = [").expect("write config");

    let err = load_dictation_config_from_path(&path, None, &env).expect_err("parse error");

    assert!(err.to_string().contains("failed to parse dictation config"));
}

#[test]
fn selected_unknown_profile_fails_fast() {
    let root = temp_root();
    let env = test_env(&root);
    let path = write_config(
        &root,
        "default_profile = \"en\"\n\n[profiles.en]\nname = \"English\"\nmodel = \"~/model.bin\"\nlanguage = \"en\"\n",
    );

    let err =
        load_dictation_config_from_path(&path, Some("fr-qc"), &env).expect_err("unknown profile");

    assert!(err
        .to_string()
        .contains("undefined dictation profile: fr-qc"));
}

#[test]
fn profile_paths_expand_home_and_xdg_variables() {
    let root = temp_root();
    let env = test_env(&root);
    let path = write_config(
        &root,
        "default_profile = \"fr-qc\"\n\n[profiles.fr-qc]\nname = \"Quebecois\"\nmodel = \"$XDG_CACHE_HOME/carlos/model.bin\"\nlanguage = \"fr\"\nvocabulary = \"~/.config/carlos/vocab-fr.txt\"\n",
    );

    let config = load_dictation_config_from_path(&path, None, &env).expect("config");
    let profile = config.profiles.get("fr-qc").expect("profile");

    assert_eq!(profile.model, root.join("cache/carlos/model.bin"));
    assert_eq!(
        profile.vocabulary.as_deref(),
        Some(root.join("home/.config/carlos/vocab-fr.txt").as_path())
    );
}

#[test]
fn model_usability_tracks_existing_file() {
    let root = temp_root();
    let env = test_env(&root);
    let model = root.join("model.bin");
    let path = write_config(
        &root,
        &format!(
            "default_profile = \"en\"\n\n[profiles.en]\nname = \"English\"\nmodel = \"{}\"\nlanguage = \"en\"\n",
            model.display()
        ),
    );

    let missing = load_dictation_config_from_path(&path, None, &env).expect("missing config");
    assert!(!missing.profiles["en"].model_is_usable());

    fs::write(&model, "model").expect("write model");
    let present = load_dictation_config_from_path(&path, None, &env).expect("present config");
    assert!(present.profiles["en"].model_is_usable());
}

fn write_config(root: &Path, text: &str) -> PathBuf {
    let path = root.join("dictation.toml");
    fs::write(&path, text).expect("write config");
    path
}

#[test]
fn raw_config_requires_profiles() {
    let raw = toml::from_str::<RawDictationConfig>("default_profile = \"en\"")
        .expect_err("missing profiles");

    assert!(raw.to_string().contains("missing field `profiles`"));
}

#[test]
fn current_environment_loader_is_exposed_for_app_startup() {
    let loader: fn(Option<&str>) -> anyhow::Result<DictationConfig> = load_dictation_config;

    let _ = loader;
}

#[test]
fn absolute_paths_do_not_require_home() {
    let root = temp_root();
    let env = DictationPathEnv {
        home: None,
        xdg_config_home: None,
        xdg_cache_home: None,
    };
    let model = root.join("model.bin");
    let path = write_config(
        &root,
        &format!(
            "default_profile = \"en\"\n\n[profiles.en]\nname = \"English\"\nmodel = \"{}\"\nlanguage = \"en\"\n",
            model.display()
        ),
    );

    let config = load_dictation_config_from_path(&path, None, &env).expect("config");

    assert_eq!(config.profiles["en"].model, model);
}
