//! Keyed registry of live PTY sessions (WP4b). Generic over the session type so
//! the id-allocation and locking semantics unit-test off-Windows; `src-tauri`
//! instantiates it with the cfg(windows) `WindowsPtySession`. Ids are
//! monotonically increasing and never reused, so a stale frontend kill of an
//! already-removed id is naturally a no-op (this subsumes the old single-session
//! generation token). The outer map lock is held only to clone the per-session
//! `Arc` out; all PTY I/O locks only that session's mutex, so one blocked
//! session never stalls commands addressed to another.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct PtyRegistry<S> {
    sessions: Mutex<HashMap<u64, Arc<Mutex<S>>>>,
    next_id: AtomicU64,
}

impl<S> Default for PtyRegistry<S> {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }
}

impl<S> PtyRegistry<S> {
    /// Register a session; returns its unique id (1-based, never reused).
    pub fn insert(&self, session: S) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.insert(id, Arc::new(Mutex::new(session)));
        id
    }

    /// Look up a live session. `None` for unknown/removed ids.
    pub fn get(&self, id: u64) -> Option<Arc<Mutex<S>>> {
        let map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.get(&id).cloned()
    }

    /// Remove a session (kill path). `None` if already removed -- idempotent.
    pub fn remove(&self, id: u64) -> Option<Arc<Mutex<S>>> {
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.remove(&id)
    }

    /// Take every live session (app shutdown).
    pub fn drain(&self) -> Vec<Arc<Mutex<S>>> {
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.drain().map(|(_, session)| session).collect()
    }
}
