//! Provision exclusive private token files and run explicit local operator commands.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use carlos_proto::devices::DeviceInfo;
use carlos_proto::ids::DeviceId;
use carlos_proto::submissions::OperationState;

use super::operator_connection::{self, Action, Reply};

enum Command {
    Add { name: String, token_file: PathBuf },
    List,
    Revoke(DeviceId),
}

struct Args {
    socket: PathBuf,
    command: Command,
}

#[derive(Default)]
struct Options {
    socket: Option<OsString>,
    name: Option<OsString>,
    token_file: Option<OsString>,
}

pub(super) fn maybe_run() -> Result<bool> {
    if std::env::args_os().nth(1).is_none_or(|arg| arg != "device") {
        return Ok(false);
    }
    let Some(args) = arguments(std::env::args_os().skip(2))? else {
        return Ok(true);
    };
    match args.command {
        Command::Add { name, token_file } => {
            let action = token(&name, &token_file)?;
            let Action::Add { device, .. } = &action else {
                unreachable!("prepared add")
            };
            let id = device.id.clone();
            confirm(&args.socket, action).with_context(|| {
                format!(
                    "Device {id} was not confirmed; keep token file {}",
                    token_file.display()
                )
            })?;
            println!("Device {id} added. Token file: {}", token_file.display());
        }
        Command::List => {
            let Reply::Devices(devices) = operator_connection::run(&args.socket, Action::List)?
            else {
                bail!("daemon returned an unexpected device-list reply");
            };
            for device in devices.devices {
                println!("{}\t{}", device.id, device.name);
            }
        }
        Command::Revoke(id) => {
            confirm(&args.socket, Action::Revoke(id.clone()))?;
            println!("Device {id} revoked");
        }
    }
    Ok(true)
}

fn confirm(socket: &Path, action: Action) -> Result<()> {
    let Reply::Operation(result) = operator_connection::run(socket, action)? else {
        bail!("daemon returned an unexpected device-write reply");
    };
    match result.state {
        OperationState::Accepted { .. } => Ok(()),
        OperationState::Rejected { message, .. } => bail!("device request rejected: {message}"),
        OperationState::Uncertain { message, .. } => bail!("device request uncertain: {message}"),
        _ => unreachable!("terminal operator result"),
    }
}

fn arguments(mut args: impl Iterator<Item = OsString>) -> Result<Option<Args>> {
    let action = args
        .next()
        .context("use carlosd device add, list or revoke")?;
    if action == "--help" || action == "-h" {
        help();
        return Ok(None);
    }
    let action = action.to_str().context("device action must be text")?;
    if !matches!(action, "add" | "list" | "revoke") {
        bail!("use carlosd device add, list or revoke");
    }
    let device = if action == "revoke" {
        Some(
            args.next()
                .context("device revoke requires an id")?
                .to_str()
                .context("device id must be text")?
                .parse()
                .context("invalid device id")?,
        )
    } else {
        None
    };
    let Some(options) = options(args)? else {
        return Ok(None);
    };
    let socket = PathBuf::from(options.socket.context("--socket is required")?);
    let command = if action == "add" {
        Command::Add {
            name: options
                .name
                .context("--name is required")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("device name must be UTF-8"))?,
            token_file: options
                .token_file
                .context("--token-file is required")?
                .into(),
        }
    } else {
        if options.name.is_some() || options.token_file.is_some() {
            bail!("--name and --token-file are only valid for device add");
        }
        device.map(Command::Revoke).unwrap_or(Command::List)
    };
    Ok(Some(Args { socket, command }))
}

fn options(mut args: impl Iterator<Item = OsString>) -> Result<Option<Options>> {
    let mut options = Options::default();
    while let Some(option) = args.next() {
        let target = match option.to_str() {
            Some("--help" | "-h") => {
                help();
                return Ok(None);
            }
            Some("--socket") => &mut options.socket,
            Some("--name") => &mut options.name,
            Some("--token-file") => &mut options.token_file,
            _ => bail!("unknown device option; use --socket, --name or --token-file"),
        };
        if target.is_some() {
            bail!("duplicate device option: {}", option.to_string_lossy());
        }
        *target = Some(args.next().context("device option requires a value")?);
    }
    Ok(Some(options))
}

fn token(name: &str, path: &Path) -> Result<Action> {
    let mut random = [0u8; 48];
    getrandom::fill(&mut random).context("obtain OS randomness for a device credential")?;
    let device = DeviceInfo {
        id: hex(&random[..16]).parse().expect("random opaque id"),
        name: name.into(),
    };
    device
        .validate()
        .map_err(|error| anyhow::anyhow!("{}", error.message))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .with_context(|| format!("create a new private token file {}", path.display()))?;
    writeln!(file, "{}.{}", device.id, hex(&random[16..]))
        .context("write the private token file")?;
    file.sync_all()
        .context("save the private token file before granting access")?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    File::open(parent)?
        .sync_all()
        .context("save the token file's directory entry")?;
    Ok(Action::Add {
        device,
        verifier: blake3::hash(&random[16..]).to_hex().to_string(),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn help() {
    println!("carlosd device add --socket PATH --name NAME --token-file NEW_PATH");
    println!("carlosd device list --socket PATH");
    println!("carlosd device revoke ID --socket PATH");
}
