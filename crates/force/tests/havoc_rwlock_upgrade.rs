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
}

#[test]
fn test_havoc_rwlock_upgrade() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());
        let mut threads = vec![];

        for _ in 0..2 {
            let manager = manager.clone();
            threads.push(thread::spawn(move || {
                manager.force_refresh();
            }));
        }

        for t in threads {
            t.join().unwrap();
        }
    });
}
