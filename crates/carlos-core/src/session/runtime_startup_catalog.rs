//! Codex model catalog additions not yet guaranteed by the app-server catalog.

use crate::protocol_params::ModelInfo;

const CODEX_DEFAULT_EFFORT: &str = "high";

const CODEX_GPT56_MODELS: [&str; 3] = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"];
const CODEX_GPT56_SUPPORTED_EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

/// Add the existing Carlos catalog entries when a native catalog needs them.
pub fn complete_codex_model_catalog(mut models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    for model in CODEX_GPT56_MODELS {
        match models.iter_mut().find(|entry| entry.model == model) {
            Some(entry) => set_codex_gpt56_efforts(entry),
            None => models.push(codex_gpt56_model_info(model)),
        }
    }
    models
}

fn codex_gpt56_model_info(model: &str) -> ModelInfo {
    let mut info = ModelInfo {
        model: model.to_string(),
        display_name: model.to_string(),
        supported_efforts: codex_gpt56_efforts(),
        default_effort: None,
        is_default: false,
    };
    set_codex_gpt56_default_effort(&mut info);
    info
}

fn set_codex_gpt56_efforts(model: &mut ModelInfo) {
    model.supported_efforts = codex_gpt56_efforts();
    set_codex_gpt56_default_effort(model);
}

fn codex_gpt56_efforts() -> Vec<String> {
    CODEX_GPT56_SUPPORTED_EFFORTS
        .iter()
        .map(|effort| (*effort).to_string())
        .collect()
}

fn set_codex_gpt56_default_effort(model: &mut ModelInfo) {
    let default_is_supported = model.default_effort.as_deref().is_some_and(|effort| {
        model
            .supported_efforts
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(effort))
    });
    if !default_is_supported {
        model.default_effort = Some(CODEX_DEFAULT_EFFORT.to_string());
    }
}
