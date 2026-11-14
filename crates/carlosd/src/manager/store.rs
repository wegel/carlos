//! Write changed launch descriptions on one bounded disk worker, never on the manager.

use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::registry::{Registry, RegistryError, SavedSession};

pub(super) struct Saved {
    pub description: SavedSession,
    pub result: Result<(), RegistryError>,
}

pub(super) struct Store {
    sender: mpsc::SyncSender<SavedSession>,
    pub completed: mpsc::Receiver<Saved>,
    stop: Arc<AtomicBool>,
}

impl Store {
    pub(super) fn start(
        path: PathBuf,
        wake: mpsc::SyncSender<()>,
        state_guard: File,
    ) -> Result<(Self, Vec<SavedSession>), RegistryError> {
        let (sender, incoming) = mpsc::sync_channel::<SavedSession>(32);
        let (done, completed) = mpsc::sync_channel(32);
        let (initial, loaded) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        thread::spawn(move || {
            // A slow disk write must keep the catalogue lock after the server stops waiting.
            let _state_guard = state_guard;
            let registry = match Registry::open(&path).and_then(|registry| {
                let rows = registry.load()?;
                Ok((registry, rows))
            }) {
                Ok((registry, rows)) => {
                    let _ = initial.send(Ok(rows));
                    registry
                }
                Err(error) => {
                    let _ = initial.send(Err(error));
                    return;
                }
            };
            while let Ok(description) = incoming.recv() {
                if worker_stop.load(Ordering::SeqCst) {
                    return;
                }
                let result = registry.save(&description);
                if done
                    .send(Saved {
                        description,
                        result,
                    })
                    .is_err()
                {
                    return;
                }
                let _ = wake.try_send(());
            }
        });
        let rows = loaded
            .recv()
            .map_err(|_| RegistryError::Invalid("catalogue worker stopped at startup".into()))??;
        Ok((
            Self {
                sender,
                completed,
                stop,
            },
            rows,
        ))
    }

    pub(super) fn save(&self, description: SavedSession) -> bool {
        self.sender.try_send(description).is_ok()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
