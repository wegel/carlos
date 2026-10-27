//! Serve disk lookups separately from the bounded queue of slow model requests.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use anyhow::Result;

use super::config::Config;
use super::provider;
use super::store::Store;
use super::{Control, Label, Request, SummaryEvent};

/// Own only bounded channels and cancellation flags on the UI thread.
pub(crate) struct SummaryService {
    tx: mpsc::SyncSender<Queued>,
    control: Arc<Control>,
}

struct Job(Request, u64, String);
struct Queued(Request, u64);
struct Finished(String, u64, Result<Label>);

impl SummaryService {
    pub fn spawn(output: mpsc::Sender<SummaryEvent>, enabled: bool) -> Self {
        Self::start(output, enabled, Config::load)
    }

    #[cfg(test)]
    pub(super) fn spawn_config(output: mpsc::Sender<SummaryEvent>, config: Config) -> Self {
        Self::start(output, true, move || Ok(config))
    }

    fn start(
        output: mpsc::Sender<SummaryEvent>,
        enabled: bool,
        load: impl FnOnce() -> Result<Config> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel(64);
        let control = Arc::new(Control {
            enabled: AtomicBool::new(enabled),
            stopped: AtomicBool::new(false),
            pid: AtomicU32::new(0),
            epoch: AtomicU64::new(0),
        });
        let worker_control = control.clone();
        thread::spawn(move || {
            if let Err(error) =
                load().and_then(|config| serve(rx, &output, &worker_control, config))
            {
                let _ = output.send(SummaryEvent::Failed(String::new(), format!("{error:#}")));
            }
        });
        Self { tx, control }
    }

    pub fn try_request(&self, request: Request) -> bool {
        self.tx
            .try_send(Queued(request, self.control.epoch.load(Ordering::Relaxed)))
            .is_ok()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.control.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.cancel_pending();
        }
    }

    pub fn cancel_pending(&self) {
        self.control.epoch.fetch_add(1, Ordering::Relaxed);
        terminate(&self.control);
    }
}

impl Drop for SummaryService {
    fn drop(&mut self) {
        self.control.stopped.store(true, Ordering::Relaxed);
        terminate(&self.control);
    }
}

fn terminate(control: &Control) {
    let pid = control.pid.load(Ordering::Relaxed);
    #[cfg(unix)]
    if pid != 0 {
        // Only the dedicated summary child ever writes this process-group ID.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

fn serve(
    rx: mpsc::Receiver<Queued>,
    output: &mpsc::Sender<SummaryEvent>,
    control: &Arc<Control>,
    config: Config,
) -> Result<()> {
    fs::create_dir_all(&config.state)?;
    let store = Store::open(&config.state.join("tool-summaries.sqlite3"))?;
    let (jobs, finished) = generation_channel(&config, control);
    let mut pending: HashMap<String, Vec<Request>> = HashMap::new();
    while !control.stopped.load(Ordering::Relaxed) {
        finish(
            &store,
            &config,
            output,
            &mut pending,
            &finished,
            control.epoch.load(Ordering::Relaxed),
        )?;
        let (mut request, request_epoch) = match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Queued(request, epoch)) => (request, epoch),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        };
        request.generate &= request_epoch == control.epoch.load(Ordering::Relaxed);
        if let Some(label) = store.lookup(&request)? {
            let _ = output.send(SummaryEvent::Ready(
                request.key,
                request.entry,
                Some(label.text),
            ));
        } else {
            queue(request, request_epoch, control, &jobs, &mut pending, output);
        }
    }
    Ok(())
}

fn generation_channel(
    config: &Config,
    control: &Arc<Control>,
) -> (mpsc::SyncSender<Job>, mpsc::Receiver<Finished>) {
    let (jobs, rx) = mpsc::sync_channel(8);
    let (tx, results) = mpsc::sync_channel(8);
    let generation_control = control.clone();
    let generation_config = config.clone();
    thread::spawn(move || generate(rx, tx, generation_config, generation_control));
    (jobs, results)
}

fn generate(
    rx: mpsc::Receiver<Job>,
    output: mpsc::SyncSender<Finished>,
    config: Config,
    control: Arc<Control>,
) {
    let mut summarizer = provider::create(&config);
    while !control.stopped.load(Ordering::Relaxed) {
        let Ok(Job(request, epoch, group)) = rx.recv() else {
            return;
        };
        let result = if control.enabled.load(Ordering::Relaxed)
            && epoch == control.epoch.load(Ordering::Relaxed)
        {
            summarizer.summarize(&request, &control, epoch)
        } else {
            Err(anyhow::anyhow!("Summary worker cancelled"))
        };
        if output.send(Finished(group, epoch, result)).is_err() {
            return;
        }
    }
}

fn finish(
    store: &Store,
    config: &Config,
    output: &mpsc::Sender<SummaryEvent>,
    pending: &mut HashMap<String, Vec<Request>>,
    finished: &mpsc::Receiver<Finished>,
    current_epoch: u64,
) -> Result<()> {
    while let Ok(Finished(group, epoch, result)) = finished.try_recv() {
        let requests = pending.remove(&group).unwrap_or_default();
        if epoch != current_epoch {
            continue;
        }
        for request in requests {
            let event = match &result {
                Ok(label) => {
                    store.save(&request, label, &config)?;
                    SummaryEvent::Ready(request.key, request.entry, Some(label.text.clone()))
                }
                Err(error) => SummaryEvent::Failed(request.entry, error.to_string()),
            };
            if output.send(event).is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn queue(
    request: Request,
    epoch: u64,
    control: &Control,
    jobs: &mpsc::SyncSender<Job>,
    pending: &mut HashMap<String, Vec<Request>>,
    output: &mpsc::Sender<SummaryEvent>,
) {
    if !request.generate || !control.enabled.load(Ordering::Relaxed) {
        let _ = output.send(SummaryEvent::Ready(request.key, request.entry, None));
        return;
    }
    let identity = if request.file_based {
        request.entry.clone()
    } else {
        blake3::Hash::from_bytes(request.key).to_hex().to_string()
    };

    let group = format!("{epoch}:{identity}");
    if let Some(waiters) = pending.get_mut(&group) {
        if waiters.len() < 128 {
            waiters.push(request);
        } else {
            let _ = output.send(SummaryEvent::Ready(request.key, request.entry, None));
        }
        return;
    }
    if jobs
        .try_send(Job(request.clone(), epoch, group.clone()))
        .is_ok()
    {
        pending.insert(group, vec![request]);
    } else {
        let _ = output.send(SummaryEvent::Ready(request.key, request.entry, None));
    }
}
