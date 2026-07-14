//! Keyed registry of live PTY sessions (WP4b). Generic over the session type so
//! the locking semantics unit-test off-Windows; `src-tauri` instantiates it
//! with the cfg(windows) `WindowsPtySession`. The outer map lock is held only
//! to clone or swap the per-session `Arc` out; all PTY I/O locks only that
//! session's mutex, so one blocked session never stalls commands addressed to
//! another.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct PtyRegistry<S> {
    sessions: Mutex<HashMap<String, Arc<Mutex<S>>>>,
}

impl<S> Default for PtyRegistry<S> {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl<S> PtyRegistry<S> {
    /// Register or replace a session by caller-owned id. Returns the previous
    /// live session when the id was already present.
    pub fn insert(&self, id: String, session: S) -> Option<Arc<Mutex<S>>> {
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.insert(id, Arc::new(Mutex::new(session)))
    }

    /// Look up a live session. `None` for unknown/removed ids.
    pub fn get(&self, id: &str) -> Option<Arc<Mutex<S>>> {
        let map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.get(id).cloned()
    }

    /// Remove a session (kill path). `None` if already removed -- idempotent.
    pub fn remove(&self, id: &str) -> Option<Arc<Mutex<S>>> {
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.remove(id)
    }

    /// Take every live session (app shutdown).
    pub fn drain(&self) -> Vec<Arc<Mutex<S>>> {
        let mut map = self.sessions.lock().expect("PtyRegistry mutex poisoned");
        map.drain().map(|(_, session)| session).collect()
    }
}
