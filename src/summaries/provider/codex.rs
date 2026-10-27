//! Call Luna through an isolated Codex home and native, read-only source tools.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{validate, Summarizer, PROMPT};
use crate::summaries::config::Config;
use crate::summaries::{process, Control, Label, Request, Source};

pub(super) struct Codex(pub Config);

impl Summarizer for Codex {
    fn summarize(
        &mut self,
        request: &Request,
        control: &Arc<Control>,
        epoch: u64,
    ) -> Result<Label> {
        let home = prepare_home(&self.0)?;
        let work = tempfile::Builder::new()
            .prefix("summary-")
            .tempdir_in(&self.0.state)?;
        let instructions = work.path().join("instructions.txt");
        let audit = work.path().join("sources.jsonl");
        fs::write(&instructions, PROMPT)?;
        let mut command = invocation(&self.0, request, &home, &instructions, &audit)?;
        let input = json!({"command":request.command}).to_string();
        let raw = process::run(&mut command, input, self.0.timeout, control, epoch)?;
        let text = parse_label(&raw)?;
        Ok(Label {
            text,
            sources: read_sources(&audit)?,
        })
    }
}

fn prepare_home(config: &Config) -> Result<PathBuf> {
    let home = config.state.join("summary-codex-home");
    fs::create_dir_all(&home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700))?;
    }
    let auth = home.join("auth.json");
    if fs::symlink_metadata(&auth).is_err() {
        let original = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|s| PathBuf::from(s).join(".codex")))
            .context("cannot locate Codex login")?
            .join("auth.json")
            .canonicalize()
            .context("Sign in to Codex with ChatGPT before enabling tool labels")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(original, &auth)?;
        #[cfg(not(unix))]
        fs::copy(original, &auth)?;
    }
    Ok(home)
}

fn invocation(
    config: &Config,
    request: &Request,
    home: &Path,
    prompt: &Path,
    audit: &Path,
) -> Result<Command> {
    let mut command = Command::new(&config.executable);
    command
        .args(&config.arguments)
        .args([
            "exec",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
            "--json",
            "--color",
            "never",
            "--strict-config",
            "-s",
            "read-only",
            "-C",
        ])
        .arg(&request.cwd)
        .arg("-m")
        .arg(&config.model);
    command.env("CODEX_HOME", home);
    minimal_config(&mut command);
    for (key, value) in [
        ("model_instructions_file", json!(prompt)),
        (
            "mcp_servers.source_reader.command",
            json!(env::current_exe()?),
        ),
        (
            "mcp_servers.source_reader.args",
            json!([
                "__summary-reader",
                request.cwd.to_string_lossy(),
                audit.to_string_lossy()
            ]),
        ),
        ("mcp_servers.source_reader.required", json!(true)),
        (
            "mcp_servers.source_reader.default_tools_approval_mode",
            json!("auto"),
        ),
        ("mcp_servers.source_reader.startup_timeout_sec", json!(5)),
        ("mcp_servers.source_reader.tool_timeout_sec", json!(2)),
    ] {
        command.arg("-c").arg(format!("{key}={value}"));
    }
    command.arg("-");
    Ok(command)
}

fn minimal_config(command: &mut Command) {
    for setting in [
        "model_reasoning_effort=\"none\"",
        "model_reasoning_summary=\"none\"",
        "service_tier=\"default\"",
        "project_doc_max_bytes=0",
        "personality=\"none\"",
        "web_search=\"disabled\"",
        "agents.enabled=false",
        "skills.bundled.enabled=false",
        "skills.include_instructions=false",
        "include_permissions_instructions=false",
        "include_environment_context=false",
        "include_collaboration_mode_instructions=false",
        "include_apps_instructions=false",
        "tools.experimental_request_user_input.enabled=false",
        "forced_login_method=\"chatgpt\"",
        "suppress_unstable_features_warning=true",
        "features.code_mode.direct_only_tool_namespaces=[\"mcp__source_reader\"]",
    ] {
        command.args(["-c", setting]);
    }
    for feature in [
        "shell_tool",
        "apps",
        "browser_use",
        "computer_use",
        "multi_agent",
        "plugins",
        "goals",
        "skill_search",
        "sleep_tool",
        "tool_suggest",
        "view_image",
        "shell_snapshot",
    ] {
        command.args(["--disable", feature]);
    }
    command.args(["--enable", "skip_host_skill_discovery"]);
}

fn parse_label(raw: &str) -> Result<String> {
    let mut label = None;
    let mut completed = false;
    for line in raw.lines() {
        let event: Value = serde_json::from_str(line)?;
        if matches!(event["type"].as_str(), Some("error" | "turn.failed")) {
            bail!("Codex summary request failed");
        }
        completed |= event["type"] == "turn.completed";
        if event["type"] != "item.completed" {
            continue;
        }
        match event["item"]["type"].as_str() {
            Some("agent_message") => label = event["item"]["text"].as_str().map(str::to_owned),
            Some("mcp_tool_call") if event["item"]["server"] == "source_reader" => {}
            Some("reasoning" | "code_mode") => {}
            _ => bail!("Unexpected tool activity in summary worker"),
        }
    }
    if !completed {
        bail!("Codex summary request did not complete");
    }
    validate(label.as_deref().context("Codex returned no summary")?)
}

fn read_sources(path: &Path) -> Result<Vec<Source>> {
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line)?;
            let hash =
                blake3::Hash::from_hex(value["hash"].as_str().context("invalid source hash")?)?;
            Ok(Source {
                path: PathBuf::from(value["path"].as_str().context("invalid source path")?),
                hash: *hash.as_bytes(),
            })
        })
        .collect()
}
