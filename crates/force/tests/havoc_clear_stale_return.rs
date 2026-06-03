#[cfg(test)]
mod tests {
    use loom::sync::{Arc, Mutex, RwLock};
    use loom::thread;

    struct TokenManager {
        token: RwLock<Option<usize>>,
        clear_count: RwLock<u64>,
        refresh_lock: Mutex<()>,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(Some(1)), // Initial soft-expired token
                clear_count: RwLock::new(0),
                refresh_lock: Mutex::new(()),
            })
        }

        fn get_soft_expired_token(&self) -> Result<usize, ()> {
            // Snapshot
            let snapshot = self.token.read().unwrap().clone();
            let valid_token = match snapshot {
                Some(t) => t,
                None => return Err(()),
            };

            // Give other threads a chance to clear and acquire refresh_lock
            thread::yield_now();

            // Try lock
            self.refresh_lock.try_lock().map_or_else(
                |_| {
                    // latest_token_or logic
                    let current = self.token.read().unwrap().clone();
                    match current {
                        Some(t) if t >= valid_token => Ok(t),
                        _ => {
                            // HAVOC: It falls back to valid_token even if token was cleared!
                            Ok(valid_token)
                        }
                    }
                },
                |_lock| {
                    // refresh
                    let new_token = 2;
                    *self.token.write().unwrap() = Some(new_token);
                    Ok(new_token)
                },
            )
        }

        fn clear(&self) {
            *self.token.write().unwrap() = None;
            *self.clear_count.write().unwrap() += 1;
        }

        fn lock_refresh(&self) -> loom::sync::MutexGuard<'_, ()> {
            self.refresh_lock.lock().unwrap()
        }
    }

    #[test]
    fn test_soft_refresh_returns_cleared_token() {
        loom::model(|| {
            let manager = TokenManager::new();
            let manager1 = manager.clone();
            let manager2 = manager.clone();

            let reader = thread::spawn(move || {
                manager1.get_soft_expired_token()
            });

            let clearer = thread::spawn(move || {
                manager2.clear();
                // keep refresh lock busy
                let _lock = manager2.lock_refresh();
                thread::yield_now();
            });

            let returned = reader.join().unwrap();
            clearer.join().unwrap();

            // If we returned 1, we returned a token that was explicitly cleared!
            if let Ok(1) = returned {
                let current_token = manager.token.read().unwrap().clone();
                assert!(current_token.is_some(), "👺 Havoc: Returned a cleared token!");
            }
        });
    }
}
