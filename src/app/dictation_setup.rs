//! Load the same device dictation profiles for embedded and socket terminals.

#[cfg(feature = "dictation")]
use super::super::dictation_state::DictationProfileState;
use super::super::view_state::ViewState;

impl ViewState {
    /// Read device preferences without starting capture or contacting the daemon.
    #[cfg(feature = "dictation")]
    pub(in crate::app) fn configure_dictation_options(&mut self, selected: Option<&str>) {
        match crate::dictation::config::load_dictation_config(selected) {
            Ok(config) => {
                let profiles = config
                    .profiles
                    .values()
                    .map(|profile| DictationProfileState {
                        id: profile.id.clone(),
                        name: profile.name.clone(),
                        model_label: Some(profile.model.display().to_string()),
                        model_usable: profile.model_is_usable(),
                        model_path: Some(profile.model.clone()),
                        language: Some(profile.language.clone()),
                        vocabulary: profile.vocabulary.clone(),
                    })
                    .collect();
                self.configure_dictation_profiles(profiles, &config.active_profile);
            }
            Err(error) => self.disable_dictation(format!("dictation unavailable: {error}")),
        }
    }

    /// Preserve the feature-disabled hint for an explicit profile selection.
    #[cfg(not(feature = "dictation"))]
    pub(in crate::app) fn configure_dictation_options(&mut self, selected: Option<&str>) {
        self.disable_dictation(if selected.is_some() {
            "dictation unavailable: rebuild with --features dictation"
        } else {
            "dictation feature is not configured"
        });
    }
}
