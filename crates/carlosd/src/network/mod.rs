//! Bound network connections and unfinished logins independently of trusted Unix peers.

mod handshake;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) use self::handshake::{upgrade, Socket};

#[derive(Default)]
pub(crate) struct Admissions {
    total: AtomicUsize,
    unauthenticated: AtomicUsize,
}

pub(crate) struct Admission {
    pool: Arc<Admissions>,
    authenticated: AtomicBool,
    pub deadline: Instant,
}

pub(crate) struct Accepted {
    pub socket: Socket,
    pub admission: Arc<Admission>,
}

impl Admissions {
    pub(crate) fn admit(self: &Arc<Self>) -> Option<Arc<Admission>> {
        if !reserve(&self.total, crate::limits::NETWORK_PEERS) {
            return None;
        }
        if !reserve(
            &self.unauthenticated,
            crate::limits::NETWORK_UNAUTHENTICATED,
        ) {
            self.total.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Arc::new(Admission {
            pool: self.clone(),
            authenticated: AtomicBool::new(false),
            deadline: Instant::now() + Duration::from_secs(5),
        }))
    }
}

fn reserve(counter: &AtomicUsize, limit: usize) -> bool {
    let mut used = counter.load(Ordering::Acquire);
    loop {
        if used >= limit {
            return false;
        }
        match counter.compare_exchange_weak(used, used + 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(current) => used = current,
        }
    }
}

impl Admission {
    pub(crate) fn authorize(&self) {
        if !self.authenticated.swap(true, Ordering::AcqRel) {
            self.pool.unauthenticated.fetch_sub(1, Ordering::AcqRel);
        }
    }

    pub(crate) fn is_authenticated(&self) -> bool {
        self.authenticated.load(Ordering::Acquire)
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        if !self.is_authenticated() {
            self.pool.unauthenticated.fetch_sub(1, Ordering::AcqRel);
        }
        self.pool.total.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests;
