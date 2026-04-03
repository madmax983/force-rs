//! Loom tests to verify TokenManager concurrency safety.
//!
//! Because `TokenManager` uses `tokio::sync::RwLock` and `tokio::sync::Mutex`,
//! we model its exact locking protocol here using `loom::sync` primitives to
//! exhaustively search for deadlocks or race conditions under all thread interleavings.

use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;

struct TokenState {
    token: Option<usize>,
}

struct TokenManagerLoomModel {
    state: Arc<RwLock<TokenState>>,
    refresh_lock: Mutex<()>,
}

impl TokenManagerLoomModel {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(TokenState { token: None })),
            refresh_lock: Mutex::new(()),
        }
    }

    // Models `TokenManager::force_refresh`
    fn force_refresh(&self) {
        let current_token = {
            let state = self.state.read().unwrap();
            state.token
        };

        let _lock = self.refresh_lock.lock().unwrap();

        {
            let state = self.state.read().unwrap();
            if let Some(token) = state.token {
                if current_token.is_some() && token > current_token.unwrap() {
                    return;
                }
            }
        }

        let mut state = self.state.write().unwrap();
        state.token = Some(current_token.unwrap_or(0) + 1);
    }

    // Models `TokenManager::clear`
    fn clear(&self) {
        let mut state = self.state.write().unwrap();
        state.token = None;
    }
}

#[test]
fn test_havoc_token_manager_loom() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());
        let mut threads = vec![];

        let m1 = manager.clone();
        threads.push(thread::spawn(move || {
            m1.force_refresh();
        }));

        let m2 = manager.clone();
        threads.push(thread::spawn(move || {
            m2.force_refresh();
        }));

        let m3 = manager.clone();
        threads.push(thread::spawn(move || {
            m3.clear();
        }));

        for t in threads {
            t.join().unwrap();
        }
    });
}
