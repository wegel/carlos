//! Dictation profile configuration loading and path resolution.

// --- Imports ---

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DictationConfig {
    pub(crate) active_profile: String,
    pub(crate) default_profile: String,
    pub(crate) profiles: BTreeMap<String, DictationProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DictationProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) model: PathBuf,
    pub(crate) language: String,
    pub(crate) vocabulary: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DictationPathEnv {
    pub(crate) home: Option<PathBuf>,
    pub(crate) xdg_config_home: Option<PathBuf>,
    pub(crate) xdg_cache_home: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RawDictationConfig {
    default_profile: Option<String>,
    profiles: BTreeMap<String, RawDictationProfile>,
}

#[derive(Debug, Deserialize)]
struct RawDictationProfile {
    name: String,
    model: String,
    language: String,
    vocabulary: Option<String>,
}

// --- Public API ---

impl DictationProfile {
    pub(crate) fn model_is_usable(&self) -> bool {
        self.model.is_file()
    }
}

impl DictationPathEnv {
    pub(crate) fn current() -> Self {
        Self {
            home: env::var_os("HOME").map(PathBuf::from),
            xdg_config_home: env::var_os("XDG_CONFIG_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            xdg_cache_home: env::var_os("XDG_CACHE_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        }
    }

    pub(crate) fn config_home(&self) -> Result<PathBuf> {
        self.xdg_config_home
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".config")))
            .context("HOME is required to locate dictation.toml")
    }

    pub(crate) fn cache_home(&self) -> Result<PathBuf> {
        self.xdg_cache_home
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".cache")))
            .context("HOME is required to locate the default dictation model")
    }
}

pub(crate) fn default_config_path(env: &DictationPathEnv) -> Result<PathBuf> {
    Ok(env.config_home()?.join("carlos").join("dictation.toml"))
}

pub(crate) fn load_dictation_config(selected_profile: Option<&str>) -> Result<DictationConfig> {
    let env = DictationPathEnv::current();
    let path = default_config_path(&env)?;
    load_dictation_config_from_path(&path, selected_profile, &env)
}

pub(crate) fn load_dictation_config_from_path(
    path: &Path,
    selected_profile: Option<&str>,
    env: &DictationPathEnv,
) -> Result<DictationConfig> {
    let mut config = if path.is_file() {
        parse_config_file(path, env)?
    } else {
        default_config(env)?
    };
    let active = selected_profile
        .map(str::to_owned)
        .unwrap_or_else(|| config.default_profile.clone());
    if !config.profiles.contains_key(&active) {
        bail!("undefined dictation profile: {active}");
    }
    config.active_profile = active;
    Ok(config)
}

// --- Parsing ---

fn parse_config_file(path: &Path, env: &DictationPathEnv) -> Result<DictationConfig> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read dictation config {}", path.display()))?;
    let raw: RawDictationConfig = toml::from_str(&text)
        .with_context(|| format!("failed to parse dictation config {}", path.display()))?;
    from_raw_config(raw, env)
}

fn from_raw_config(raw: RawDictationConfig, env: &DictationPathEnv) -> Result<DictationConfig> {
    if raw.profiles.is_empty() {
        bail!("dictation config must define at least one profile");
    }
    let default_profile = raw
        .default_profile
        .unwrap_or_else(|| raw.profiles.keys().next().cloned().unwrap_or_default());
    let profiles = raw
        .profiles
        .into_iter()
        .map(|(id, profile)| expand_profile(id, profile, env))
        .collect::<Result<BTreeMap<_, _>>>()?;

    Ok(DictationConfig {
        active_profile: default_profile.clone(),
        default_profile,
        profiles,
    })
}

fn expand_profile(
    id: String,
    raw: RawDictationProfile,
    env: &DictationPathEnv,
) -> Result<(String, DictationProfile)> {
    let model = expand_path(&raw.model, env)?;
    let vocabulary = raw
        .vocabulary
        .as_deref()
        .map(|path| expand_path(path, env))
        .transpose()?;
    let profile = DictationProfile {
        id: id.clone(),
        name: raw.name,
        model,
        language: raw.language,
        vocabulary,
    };
    Ok((id, profile))
}

fn default_config(env: &DictationPathEnv) -> Result<DictationConfig> {
    let model = env.cache_home()?.join("carlos").join("whisper-model.bin");
    let profile = DictationProfile {
        id: "en".to_string(),
        name: "English".to_string(),
        model,
        language: "en".to_string(),
        vocabulary: None,
    };
    let mut profiles = BTreeMap::new();
    profiles.insert(profile.id.clone(), profile);
    Ok(DictationConfig {
        active_profile: "en".to_string(),
        default_profile: "en".to_string(),
        profiles,
    })
}

// --- Path Expansion ---

fn expand_path(value: &str, env: &DictationPathEnv) -> Result<PathBuf> {
    let mut expanded = value.to_string();
    if expanded.contains("$XDG_CONFIG_HOME") {
        expanded = replace_env_var(&expanded, "$XDG_CONFIG_HOME", &env.config_home()?);
    }
    if expanded.contains("$XDG_CACHE_HOME") {
        expanded = replace_env_var(&expanded, "$XDG_CACHE_HOME", &env.cache_home()?);
    }
    if expanded.contains("$HOME") {
        let home = env.home.as_ref().context("HOME is required for $HOME")?;
        expanded = replace_env_var(&expanded, "$HOME", home);
    }
    if expanded == "~" {
        return env.home.clone().context("HOME is required for ~");
    }
    if let Some(rest) = expanded.strip_prefix("~/") {
        let home = env.home.as_ref().context("HOME is required for ~/ paths")?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(expanded))
}

fn replace_env_var(input: &str, name: &str, value: &Path) -> String {
    input.replace(name, &value.to_string_lossy())
}

// --- Tests ---

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
