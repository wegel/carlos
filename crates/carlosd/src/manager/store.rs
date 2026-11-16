//! Save session descriptions and device grants on one bounded disk worker, never on the manager.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use super::devices::{DeviceChange, DeviceWrite};
use crate::registry::{Registry, RegistryError, SavedDevice, SavedSession};

pub(super) enum Write {
    Session(SavedSession),
    Device(DeviceWrite),
}

pub(super) struct Loaded {
    pub sessions: Vec<SavedSession>,
    pub devices: Vec<SavedDevice>,
}

pub(super) struct Saved {
    pub request: Write,
    pub result: Result<(), RegistryError>,
}

pub(super) struct Store {
    sender: mpsc::SyncSender<Write>,
    completed: mpsc::Receiver<Saved>,
    stop: Arc<AtomicBool>,
    pub failed: bool,
}

impl Store {
    pub(super) fn start(
        path: PathBuf,
        wake: mpsc::SyncSender<()>,
        state_guard: File,
    ) -> Result<(Self, Loaded), RegistryError> {
        let (sender, incoming) = mpsc::sync_channel(32);
        let (done, completed) = mpsc::sync_channel(32);
        let (initial, loaded) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        thread::spawn(move || {
            // A slow disk write must keep the catalogue lock after the server stops waiting.
            let _state_guard = state_guard;
            let registry = match load(&path) {
                Ok((registry, rows)) => {
                    let _ = initial.send(Ok(rows));
                    registry
                }
                Err(error) => {
                    let _ = initial.send(Err(error));
                    return;
                }
            };
            serve(registry, incoming, done, worker_stop, wake);
        });
        let rows = loaded
            .recv()
            .map_err(|_| RegistryError::Invalid("catalogue worker stopped at startup".into()))??;
        Ok((
            Self {
                sender,
                completed,
                stop,
                failed: false,
            },
            rows,
        ))
    }

    pub(super) fn save(&self, description: SavedSession) -> bool {
        self.submit(Write::Session(description))
    }

    pub(super) fn save_device(&self, write: DeviceWrite) -> bool {
        self.submit(Write::Device(write))
    }

    fn submit(&self, request: Write) -> bool {
        !self.failed && self.sender.try_send(request).is_ok()
    }

    pub(super) fn take(&mut self) -> Result<Option<Saved>, ()> {
        if self.failed {
            return Ok(None);
        }
        match self.completed.try_recv() {
            Ok(saved) => Ok(Some(saved)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.failed = true;
                Err(())
            }
        }
    }
}

fn load(path: &Path) -> Result<(Registry, Loaded), RegistryError> {
    let registry = Registry::open(path)?;
    let rows = Loaded {
        sessions: registry.load()?,
        devices: registry.load_devices()?,
    };
    Ok((registry, rows))
}

fn serve(
    registry: Registry,
    incoming: mpsc::Receiver<Write>,
    done: mpsc::SyncSender<Saved>,
    stop: Arc<AtomicBool>,
    wake: mpsc::SyncSender<()>,
) {
    while let Ok(request) = incoming.recv() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let result = match &request {
            Write::Session(description) => registry.save(description),
            Write::Device(write) => match &write.change {
                DeviceChange::Add(device) => registry.save_device(device),
                DeviceChange::Revoke(id) => registry.revoke_device(id),
            },
        };
        if done.send(Saved { request, result }).is_err() {
            return;
        }
        let _ = wake.try_send(());
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
