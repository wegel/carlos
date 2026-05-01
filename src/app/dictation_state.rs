//! App-side dictation state machine and display labels.

// --- Imports ---

#[cfg(feature = "dictation")]
use std::path::PathBuf;

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DictationPhase {
    Disabled,
    Idle,
    Recording,
    Transcribing { partial: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DictationEndpointMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DictationProfileState {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) model_label: Option<String>,
    pub(super) model_usable: bool,
    #[cfg(feature = "dictation")]
    pub(super) model_path: Option<PathBuf>,
    #[cfg(feature = "dictation")]
    pub(super) language: Option<String>,
    #[cfg(feature = "dictation")]
    pub(super) vocabulary: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DictationRuntimeState {
    phase: DictationPhase,
    endpoint_mode: DictationEndpointMode,
    profile: Option<DictationProfileState>,
    #[cfg(feature = "dictation")]
    profiles: Vec<DictationProfileState>,
    picker_open: bool,
    disabled_reason: Option<String>,
}

// --- Lifecycle ---

impl DictationRuntimeState {
    pub(super) fn disabled(reason: impl Into<String>) -> Self {
        Self {
            phase: DictationPhase::Disabled,
            endpoint_mode: DictationEndpointMode::Auto,
            profile: None,
            #[cfg(feature = "dictation")]
            profiles: Vec::new(),
            picker_open: false,
            disabled_reason: Some(reason.into()),
        }
    }

    #[cfg(test)]
    pub(super) fn with_profile(profile: DictationProfileState) -> Self {
        Self {
            phase: DictationPhase::Idle,
            endpoint_mode: DictationEndpointMode::Auto,
            profile: Some(profile.clone()),
            #[cfg(feature = "dictation")]
            profiles: vec![profile],
            picker_open: false,
            disabled_reason: None,
        }
    }

    #[cfg(feature = "dictation")]
    pub(super) fn with_profiles(profiles: Vec<DictationProfileState>, active_id: &str) -> Self {
        let profile = profiles
            .iter()
            .find(|profile| profile.id == active_id)
            .cloned()
            .or_else(|| profiles.first().cloned());
        Self {
            phase: if profile.is_some() {
                DictationPhase::Idle
            } else {
                DictationPhase::Disabled
            },
            endpoint_mode: DictationEndpointMode::Auto,
            profile,
            profiles,
            picker_open: false,
            disabled_reason: None,
        }
    }

    #[cfg(test)]
    pub(super) fn phase(&self) -> &DictationPhase {
        &self.phase
    }

    #[cfg(feature = "dictation")]
    pub(super) fn profile(&self) -> Option<&DictationProfileState> {
        self.profile.as_ref()
    }

    #[cfg(feature = "dictation")]
    pub(super) fn cycle_profile(&mut self) -> Result<&DictationProfileState, String> {
        if self.profiles.is_empty() {
            return Err("dictation profile list is empty".to_string());
        }
        let current_idx = self
            .profile
            .as_ref()
            .and_then(|active| {
                self.profiles
                    .iter()
                    .position(|profile| profile.id == active.id)
            })
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.profiles.len();
        self.profile = Some(self.profiles[next_idx].clone());
        self.phase = DictationPhase::Idle;
        Ok(self.profile.as_ref().expect("profile was just selected"))
    }

    pub(super) fn is_active(&self) -> bool {
        matches!(
            self.phase,
            DictationPhase::Recording | DictationPhase::Transcribing { .. }
        )
    }

    pub(super) fn is_recording(&self) -> bool {
        matches!(self.phase, DictationPhase::Recording)
    }

    #[cfg(feature = "dictation")]
    pub(super) fn auto_endpoint_enabled(&self) -> bool {
        self.endpoint_mode == DictationEndpointMode::Auto
    }

    pub(super) fn status_label(&self) -> Option<String> {
        let profile = self.profile.as_ref()?;
        match &self.phase {
            DictationPhase::Recording => Some(format!(
                "DICTATING {} [{}]",
                self.endpoint_mode.label().to_ascii_uppercase(),
                profile.name
            )),
            DictationPhase::Transcribing { .. } => Some(format!("TRANSCRIBING [{}]", profile.name)),
            DictationPhase::Disabled | DictationPhase::Idle => None,
        }
    }

    pub(super) fn endpoint_mode_label(&self) -> &'static str {
        self.endpoint_mode.label()
    }
}

// --- Transitions ---

impl DictationRuntimeState {
    pub(super) fn start_recording(&mut self) -> Result<(), String> {
        let profile = self.profile.as_ref().ok_or_else(|| {
            self.disabled_reason
                .clone()
                .unwrap_or_else(|| "dictation unavailable".to_string())
        })?;
        if !profile.model_usable {
            let model = profile.model_label.as_deref().unwrap_or("configured model");
            return Err(format!("dictation model unavailable: {model}"));
        }
        self.phase = DictationPhase::Recording;
        Ok(())
    }

    pub(super) fn stop_recording(&mut self) -> bool {
        if !matches!(self.phase, DictationPhase::Recording) {
            return false;
        }
        self.phase = DictationPhase::Transcribing {
            partial: String::new(),
        };
        true
    }

    pub(super) fn cancel(&mut self) -> bool {
        if !self.is_active() {
            return false;
        }
        self.phase = if self.profile.is_some() {
            DictationPhase::Idle
        } else {
            DictationPhase::Disabled
        };
        true
    }

    pub(super) fn toggle_endpoint_mode(&mut self) -> Result<DictationEndpointMode, String> {
        if self.profile.is_none() {
            return Err(self
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "dictation unavailable".to_string()));
        }
        self.endpoint_mode = match self.endpoint_mode {
            DictationEndpointMode::Auto => DictationEndpointMode::Manual,
            DictationEndpointMode::Manual => DictationEndpointMode::Auto,
        };
        Ok(self.endpoint_mode)
    }

    #[cfg(test)]
    pub(super) fn apply_partial(&mut self, text: impl Into<String>) -> bool {
        let DictationPhase::Transcribing { partial } = &mut self.phase else {
            return false;
        };
        *partial = text.into();
        true
    }

    #[cfg(any(test, feature = "dictation"))]
    pub(super) fn finish_transcription(&mut self) -> Option<String> {
        let DictationPhase::Transcribing { partial } =
            std::mem::replace(&mut self.phase, DictationPhase::Idle)
        else {
            return None;
        };
        Some(partial)
    }

    #[cfg(test)]
    pub(super) fn open_picker(&mut self) {
        self.picker_open = true;
    }

    #[cfg(test)]
    pub(super) fn close_picker(&mut self) {
        self.picker_open = false;
    }

    #[cfg(test)]
    pub(super) fn picker_open(&self) -> bool {
        self.picker_open
    }
}

impl DictationEndpointMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            DictationEndpointMode::Auto => "auto",
            DictationEndpointMode::Manual => "manual",
        }
    }
}
