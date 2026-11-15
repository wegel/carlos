//! Resolve captured startup choices without reading files or calling a backend.

use crate::protocol_params::{ModelInfo, ThreadRuntimeSettings};
use crate::session::runtime_settings::RuntimeDefaults;

const CODEX_DEFAULT_MODEL: &str = "gpt-5.5";
const CODEX_DEFAULT_EFFORT: &str = "high";
const CODEX_DEFAULT_SUMMARY: &str = "concise";
const GROK_DEFAULT_MODEL: &str = "grok-4.6";
const GROK_DEFAULT_EFFORT: &str = "high";

/// Supply Carlos fallbacks only when native and saved choices are absent.
pub fn codex_runtime_defaults(default_summary: Option<String>) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(CODEX_DEFAULT_MODEL.to_string()),
        effort: Some(CODEX_DEFAULT_EFFORT.to_string()),
        summary: default_summary.or_else(|| Some(CODEX_DEFAULT_SUMMARY.to_string())),
    }
}

/// Keep unspecified Claude choices under native control.
pub fn claude_runtime_defaults() -> RuntimeDefaults {
    RuntimeDefaults::default()
}

/// Supply the existing Carlos Grok model and effort fallbacks.
pub fn grok_runtime_defaults() -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(GROK_DEFAULT_MODEL.to_string()),
        effort: Some(GROK_DEFAULT_EFFORT.to_string()),
        summary: None,
    }
}

/// Resolve native and saved choices against the advertised model and effort list.
pub fn resolve_backend_runtime_settings(
    runtime: ThreadRuntimeSettings,
    persisted_defaults: &RuntimeDefaults,
    fallback_defaults: &RuntimeDefaults,
    models: &[ModelInfo],
    supports_summary: bool,
) -> ThreadRuntimeSettings {
    let requested_model = runtime
        .model
        .or_else(|| persisted_defaults.model.clone())
        .or_else(|| fallback_defaults.model.clone());
    let model = choose_compatible_model(
        requested_model.as_deref(),
        fallback_defaults.model.as_deref(),
        models,
    );
    let model_info = model
        .as_deref()
        .and_then(|selected| find_model(models, selected));
    let model_uses_requested = requested_model.as_deref() == model.as_deref();
    let requested_effort = if model_uses_requested {
        runtime
            .effort
            .or_else(|| persisted_defaults.effort.clone())
            .or_else(|| fallback_defaults.effort.clone())
    } else {
        fallback_defaults.effort.clone()
    };
    let effort = choose_compatible_effort(
        requested_effort.as_deref(),
        fallback_defaults.effort.as_deref(),
        model_info,
    );
    let summary = supports_summary
        .then(|| {
            runtime
                .summary
                .or_else(|| persisted_defaults.summary.clone())
                .or_else(|| fallback_defaults.summary.clone())
        })
        .flatten();
    ThreadRuntimeSettings {
        model,
        effort,
        summary,
    }
}

fn find_model<'a>(models: &'a [ModelInfo], model: &str) -> Option<&'a ModelInfo> {
    models.iter().find(|candidate| candidate.model == model)
}

fn choose_compatible_model(
    requested: Option<&str>,
    fallback: Option<&str>,
    models: &[ModelInfo],
) -> Option<String> {
    if models.is_empty() {
        return requested.or(fallback).map(ToOwned::to_owned);
    }
    requested
        .and_then(|model| find_model(models, model))
        .or_else(|| fallback.and_then(|model| find_model(models, model)))
        .or_else(|| models.iter().find(|model| model.is_default))
        .or_else(|| models.first())
        .map(|model| model.model.clone())
}

fn choose_compatible_effort(
    requested: Option<&str>,
    fallback: Option<&str>,
    model: Option<&ModelInfo>,
) -> Option<String> {
    let Some(model) = model else {
        return requested.or(fallback).map(ToOwned::to_owned);
    };
    if model.supported_efforts.is_empty() {
        return requested.or(fallback).map(ToOwned::to_owned);
    }
    requested
        .and_then(|effort| find_effort(&model.supported_efforts, effort))
        .or_else(|| fallback.and_then(|effort| find_effort(&model.supported_efforts, effort)))
        .or_else(|| {
            model
                .default_effort
                .as_deref()
                .and_then(|effort| find_effort(&model.supported_efforts, effort))
        })
        .or_else(|| model.supported_efforts.first().cloned())
}

fn find_effort(efforts: &[String], effort: &str) -> Option<String> {
    efforts
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(effort))
        .cloned()
}
