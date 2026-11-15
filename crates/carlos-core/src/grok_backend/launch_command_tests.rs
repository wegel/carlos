//! Keep exact native command flags and the Grok catalog at their backend helpers.

use std::path::Path;

use super::build_grok_command;
use crate::grok_backend::{grok_model_catalog, GrokRuntimeSettings};

#[test]
fn build_grok_command_uses_agent_stdio_always_approve() {
    let command = build_grok_command(Path::new("/repo"), &GrokRuntimeSettings::default());
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(args, ["agent", "--always-approve", "--no-leader", "stdio"]);
}

#[test]
fn build_grok_command_includes_model_and_effort() {
    let command = build_grok_command(
        Path::new("/repo"),
        &GrokRuntimeSettings::new(Some("grok-4.5".to_string()), Some("medium".to_string())),
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args
        .windows(2)
        .any(|window| window == ["--model", "grok-4.5"]));
    assert!(args
        .windows(2)
        .any(|window| window == ["--reasoning-effort", "medium"]));
}

#[test]
fn grok_model_catalog_includes_current_models() {
    let catalog = grok_model_catalog();
    let models = catalog
        .iter()
        .map(|model| model.model.as_str())
        .collect::<Vec<_>>();
    assert_eq!(models.first().copied(), Some("grok-4.6"));
    assert!(catalog.first().is_some_and(|model| model.is_default));
    assert!(models.contains(&"grok-4.5"));
    assert_eq!(
        catalog[0].supported_efforts,
        ["xhigh", "high", "medium", "low"]
    );
}
