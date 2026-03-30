//! Havoc regression test for concurrent token stall during soft expiry.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::collapsible_if)]
    #![allow(clippy::option_if_let_else)]
    // Use std so the test reliably runs and demonstrates the failure without needing `RUSTFLAGS="--cfg loom"`
    use loom::sync::{Arc, Mutex, RwLock};
    use loom::thread;

    // Simplified TokenManager logic for Loom testing
    // This mirrors the logic in crates/force/src/auth/token_manager.rs
    struct TokenManager {
        token: RwLock<Option<String>>,
        refresh_lock: Mutex<()>,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(None),
                refresh_lock: Mutex::new(()),
            })
        }

        fn get_token(&self) -> String {
            // Fast path: read lock
            {
                let guard = self.token.read().unwrap();
                if let Some(token) = &*guard {
                    if token != "soft_expired" {
                        return token.clone();
                    }
                }
            } // Read lock drops here

            // Determine state (BUG: we reacquire the read lock here)
            // If someone is holding the refresh_lock and refreshing, they will eventually
            // need the write lock to update the state.
            let (is_hard_expired, current_token) = {
                let guard = self.token.read().unwrap();
                if let Some(token) = &*guard {
                    (token == "hard_expired", Some(token.clone()))
                } else {
                    (true, None)
                }
            };

            if is_hard_expired {
                // Hard expired path - block
                let _lock = self.refresh_lock.lock().unwrap();

                // Double check
                {
                    let guard = self.token.read().unwrap();
                    if let Some(token) = &*guard {
                        if token != "hard_expired" {
                            return token.clone();
                        }
                    }
                }

                let new_token = "new_token".to_string();
                let mut guard = self.token.write().unwrap();
                *guard = Some(new_token.clone());
                new_token
            } else if let Some(valid_token) = current_token {
                // Soft expired path
                if let Ok(_lock) = self.refresh_lock.try_lock() {
                    let new_token = "new_token".to_string();
                    let mut guard = self.token.write().unwrap();
                    *guard = Some(new_token.clone());
                    new_token
                } else {
                    valid_token
                }
            } else {
                panic!("Unreachable");
            }
        }
    }

    #[test]
    fn test_concurrent_readers_dont_stall_during_soft_expiry() {
        loom::model(|| {
            let manager = TokenManager::new();

            // Set up soft expired token
            {
                let mut guard = manager.token.write().unwrap();
                *guard = Some("soft_expired".to_string());
            }

            let m1 = manager.clone();
            let m2 = manager;

            // Thread 1 will try to read, see soft expiry, grab refresh_lock
            let t1 = thread::spawn(move || m1.get_token());

            // Thread 2 will also try to read. It should be able to read and return
            // the soft_expired token immediately without blocking on Thread 1
            let t2 = thread::spawn(move || m2.get_token());

            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            assert!(r1 == "new_token" || r1 == "soft_expired");
            assert!(r2 == "new_token" || r2 == "soft_expired");
        });
    }
}
