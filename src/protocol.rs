use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

pub(crate) struct AppServerClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    events_rx: Option<mpsc::Receiver<String>>,
    next_id: AtomicU64,
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl AppServerClient {
    pub(crate) fn start() -> Result<Self> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn `codex app-server`")?;

        let stdin = child.stdin.take().context("missing child stdin")?;
        let stdout = child.stdout.take().context("missing child stdout")?;

        let (events_tx, events_rx) = mpsc::channel::<String>();
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_for_thread = Arc::clone(&pending);

        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                let n = match reader.read_line(&mut line) {
                    Ok(n) => n,
                    Err(_) => break,
                };
                if n == 0 {
                    break;
                }
                let trimmed = line.trim_end_matches(['\n', '\r']);
                if trimmed.is_empty() {
                    continue;
                }

                let parsed: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(method) = parsed.get("method").and_then(Value::as_str) {
                    if method.starts_with("codex/event/")
                        && !matches!(
                            method,
                            "codex/event/raw_response_item"
                                | "codex/event/exec_command_end"
                                | "codex/event/turn_diff"
                                | "codex/event/token_count"
                        )
                    {
                        continue;
                    }
                }

                if parsed.get("method").is_none() {
                    if let Some(id) = json_id_to_u64(parsed.get("id")) {
                        if let Some(tx) = pending_for_thread
                            .lock()
                            .ok()
                            .and_then(|mut p| p.remove(&id))
                        {
                            let _ = tx.send(trimmed.to_string());
                            continue;
                        }
                    }
                }

                let _ = events_tx.send(trimmed.to_string());
            }
        });

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            events_rx: Some(events_rx),
            next_id: AtomicU64::new(1),
            reader_thread: Some(reader_thread),
        })
    }

    pub(crate) fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel::<String>();

        self.pending
            .lock()
            .map_err(|_| anyhow!("pending lock poisoned"))?
            .insert(id, tx);

        let line = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|_| anyhow!("stdin lock poisoned"))?;
            stdin.write_all(line.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()?;
        }

        match rx.recv_timeout(timeout) {
            Ok(resp) => Ok(resp),
            Err(_) => {
                let _ = self.pending.lock().map(|mut p| p.remove(&id));
                bail!("timeout waiting for {method}");
            }
        }
    }

    pub(crate) fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("events receiver already taken"))
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
        let _ = self.reader_thread.take();
    }
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        self.stop();
    }
}

fn json_id_to_u64(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return (n >= 0).then_some(n as u64);
    }
    if let Some(n) = v.as_f64() {
        return (n >= 0.0).then_some(n as u64);
    }
    v.as_str()?.parse::<u64>().ok()
}

pub(crate) fn params_initialize() -> Value {
    json!({
        "clientInfo": {
            "name": "carlos",
            "title": "carlos",
            "version": "0.1.0"
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

pub(crate) fn params_thread_start(cwd: &str) -> Value {
    json!({
        "experimentalRawEvents": false,
        "persistExtendedHistory": true,
        "cwd": cwd,
    })
}

pub(crate) fn params_thread_resume(thread_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "persistExtendedHistory": true,
    })
}

pub(crate) fn params_thread_list(cwd: &str) -> Value {
    json!({
        "limit": 100,
        "cwd": cwd,
    })
}

pub(crate) fn params_turn_start(thread_id: &str, text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    })
}

pub(crate) fn params_turn_steer(thread_id: &str, turn_id: &str, text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "expectedTurnId": turn_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    })
}

pub(crate) fn initialize_client(client: &AppServerClient) -> Result<()> {
    let resp = client.call("initialize", params_initialize(), Duration::from_secs(10))?;
    extract_result_object(&resp)?;
    Ok(())
}

pub(crate) fn extract_result_object(line: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(line).context("invalid JSON response")?;
    if parsed.get("error").is_some() {
        bail!(
            "server returned error: {}",
            parsed.get("error").unwrap_or(&Value::Null)
        );
    }
    if parsed.get("result").is_none() {
        bail!("missing result in response");
    }
    Ok(parsed)
}

pub(crate) fn parse_thread_id_from_start_or_resume(response_line: &str) -> Result<String> {
    let parsed = extract_result_object(response_line)?;
    let result = parsed
        .get("result")
        .and_then(Value::as_object)
        .context("invalid result object")?;
    let thread = result
        .get("thread")
        .and_then(Value::as_object)
        .context("missing thread in result")?;
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .context("missing thread.id")?;
    Ok(id.to_string())
}
