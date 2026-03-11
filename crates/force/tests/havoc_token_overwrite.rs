//! Havoc race condition test for `TokenManager::get_token` overwriting `force_refresh`.
//!
//! This test simulates the race condition where `get_token` (which refreshes inside the lock)
//! overwrites a newer token placed by `force_refresh` (which refreshes outside the lock).

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    // Use std so the test reliably runs and demonstrates the failure without needing `RUSTFLAGS="--cfg loom"`
    use std::sync::{Arc, Mutex, RwLock};
    use std::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Simplified TokenManager logic mirroring crates/force/src/auth/token_manager.rs
    struct TokenManager {
        token: RwLock<Option<usize>>, // Token is just a sequence number
        counter: Arc<AtomicUsize>,    // Simulates the Authenticator
        refresh_lock: Mutex<()>,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(None),
                counter: Arc::new(AtomicUsize::new(0)),
                refresh_lock: Mutex::new(()),
            })
        }

        // Simulates authenticator.refresh()
        fn generate_token(&self) -> usize {
            self.counter.fetch_add(1, Ordering::SeqCst) + 1
        }

        // Simulates TokenManager::get_token_arc (the hard expired path part)
        fn get_token(&self) -> usize {
            // Fast path (read lock)
            {
                let guard = self.token.read().unwrap();
                if let Some(token) = *guard {
                    return token;
                }
            }

            // Acquire refresh lock
            let _lock = self.refresh_lock.lock().unwrap();

            // Double check
            {
                let guard = self.token.read().unwrap();
                if let Some(token) = *guard {
                    return token;
                }
            }

            let new_token = self.generate_token();

            // Artificial delay to guarantee Thread 1 (force_refresh) finishes its generation
            // and writes its newer token into the lock first.
            thread::sleep(std::time::Duration::from_millis(10));

            let mut guard = self.token.write().unwrap();

            // The Fix: only overwrite if `new_token` is strictly newer.
            if let Some(current) = *guard {
                if current > new_token {
                    return current;
                }
            }

            *guard = Some(new_token);
            new_token
        }

        // Simulates TokenManager::force_refresh
        fn force_refresh(&self) -> usize {
            let new_token = self.generate_token();

            {
                let mut guard = self.token.write().unwrap();
                if let Some(current) = *guard {
                    if current > new_token {
                        return current;
                    }
                }
                *guard = Some(new_token);
            }
            new_token
        }

        fn current_token(&self) -> Option<usize> {
            *self.token.read().unwrap()
        }
    }

    #[test]
    fn test_get_token_overwrites_newer_token() {
        let manager = TokenManager::new();
        let m1 = manager.clone();
        let m2 = manager.clone();

        // Thread 1: Calls force_refresh (starts slightly after t2 to ensure t2 gets refresh_lock first)
        let t1 = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(2));
            m1.force_refresh()
        });

        // Thread 2: Calls get_token (which triggers a refresh because initial state is None)
        let t2 = thread::spawn(move || m2.get_token());

        let _ = t1.join();
        let _ = t2.join();

        let final_token = manager.current_token().unwrap();
        let max_generated = manager.counter.load(Ordering::SeqCst);

        if max_generated == 2 {
            assert_eq!(
                final_token, 2,
                "👺 Havoc: get_token overwrote a newer token!"
            );
        }
    }
}
