//! Give the summary model bounded source reads, without shell or write tools.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

const MAX_BYTES: usize = 65_536;

pub(super) struct Reader {
    root: PathBuf,
    audit: PathBuf,
    calls: usize,
    bytes: usize,
}

impl Reader {
    pub fn new(root: &Path, audit: &Path) -> Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
            audit: audit.to_owned(),
            calls: 0,
            bytes: 0,
        })
    }

    pub fn call(&mut self, name: &str, path: &str) -> Result<String> {
        self.calls += 1;
        if self.calls > 4 {
            bail!("Four-call source inspection budget exhausted");
        }
        let path = self.root.join(path).canonicalize()?;
        if !path.starts_with(&self.root) {
            bail!("Path outside command workspace");
        }
        match name {
            "read_file" => self.read(&path),
            "list_directory" => {
                let entries = fs::read_dir(path)?
                    .take(40)
                    .map(|e| e.map(|entry| entry.file_name().to_string_lossy().into_owned()))
                    .collect::<io::Result<Vec<_>>>()?;
                Ok(entries.join("\n"))
            }
            _ => bail!("Unsupported source tool"),
        }
    }

    fn read(&mut self, path: &Path) -> Result<String> {
        let file = open_source(path)?;
        if !file.metadata()?.is_file() {
            bail!("Not a regular source file");
        }
        let mut raw = Vec::new();
        file.take((MAX_BYTES - self.bytes + 1) as u64)
            .read_to_end(&mut raw)?;
        if self.bytes + raw.len() > MAX_BYTES {
            bail!("64 KiB source budget exceeded");
        }
        let text = String::from_utf8(raw.clone()).context("Source is not UTF-8")?;
        self.bytes += raw.len();
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit)?;
        writeln!(
            log,
            "{}",
            json!({"path": path.strip_prefix(&self.root)?,
            "hash": blake3::hash(&raw).to_hex().to_string()})
        )?;
        Ok(text)
    }
}

pub(super) fn open_source(path: &Path) -> Result<fs::File> {
    if !fs::metadata(path)?.is_file() {
        bail!("Not a regular source file");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    Ok(options.open(path)?)
}

fn tools() -> Value {
    let tools = ["read_file", "list_directory"].into_iter().map(|name| json!({
        "name": name, "description": if name == "read_file" { "Read UTF-8 source, never execute it" } else { "List a source directory" },
        "inputSchema": {"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false},
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}
    })).collect::<Vec<_>>();
    json!({"tools": tools})
}

fn dispatch(request: &Value, reader: &mut Reader) -> Result<Value> {
    let params = &request["params"];
    Ok(match request["method"].as_str().unwrap_or("") {
        "initialize" => {
            json!({"protocolVersion":params["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"carlos-source-reader","version":"1"}})
        }
        "tools/list" => tools(),
        "ping" => json!({}),
        "tools/call" => {
            let result = reader.call(
                params["name"].as_str().unwrap_or(""),
                params["arguments"]["path"]
                    .as_str()
                    .context("Source path must be a string")?,
            );
            let failed = result.is_err();
            let text = result.unwrap_or_else(|e| e.to_string());
            json!({"content":[{"type":"text","text":text}],"isError":failed})
        }
        _ => bail!("Unsupported MCP method"),
    })
}

/// Handle only the private reader subcommand before starting a terminal/backend.
pub(crate) fn maybe_run() -> Result<bool> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("__summary-reader")) {
        return Ok(false);
    }
    let root = args.next().context("missing source workspace")?;
    let audit = args.next().context("missing source audit path")?;
    let mut reader = Reader::new(Path::new(&root), Path::new(&audit))?;
    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        if request.get("id").is_none() {
            continue;
        }
        let response = match dispatch(&request, &mut reader) {
            Ok(value) => json!({"jsonrpc":"2.0","id":request["id"],"result":value}),
            Err(error) => {
                json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32601,"message":error.to_string()}})
            }
        };
        println!("{response}");
        io::stdout().flush()?;
    }
    Ok(true)
}
