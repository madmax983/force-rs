//! Havoc race condition test for TokenManager::force_refresh.
//!
//! This test simulates the race condition where `force_refresh` (which refreshes outside the lock)
//! competes with `get_token` (which refreshes inside the lock).

#[cfg(test)]
mod tests {
    use loom::sync::{Arc, RwLock};
    use loom::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Simplified TokenManager logic mirroring crates/force/src/auth/token_manager.rs
    struct TokenManager {
        token: RwLock<Option<usize>>, // Token is just a sequence number
        counter: Arc<AtomicUsize>,    // Simulates the Authenticator
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(None),
                counter: Arc::new(AtomicUsize::new(0)),
            })
        }

        // Simulates authenticator.refresh()
        fn generate_token(&self) -> usize {
            // In the real world, this takes time (network request)
            // Loom will explore context switches here if we do something observable?
            // AtomicUsize fetch_add is atomic, but the interleaving of this vs lock acquisition is what matters.
            self.counter.fetch_add(1, Ordering::SeqCst) + 1
        }

        // Simulates TokenManager::get_token_arc (the slow path part)
        fn get_token(&self) -> usize {
            // Fast path (read lock)
            {
                let guard = self.token.read().unwrap();
                if let Some(token) = *guard {
                    return token;
                }
            }

            // Slow path (write lock)
            let mut guard = self.token.write().unwrap();

            // Double check
            if let Some(token) = *guard {
                return token;
            }

            // Refresh INSIDE lock
            let new_token = self.generate_token();
            *guard = Some(new_token);
            new_token
        }

        // Simulates TokenManager::force_refresh (with FIX)
        fn force_refresh(&self) -> usize {
            // Refresh OUTSIDE lock
            let new_token = self.generate_token();

            // Update state INSIDE lock
            {
                let mut guard = self.token.write().unwrap();
                // FIX: Check if current token is newer
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
    fn test_force_refresh_race_condition() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            // Thread 1: Calls force_refresh
            let t1 = thread::spawn(move || {
                m1.force_refresh()
            });

            // Thread 2: Calls get_token (which triggers a refresh because initial state is None)
            let t2 = thread::spawn(move || {
                m2.get_token()
            });

            let _ = t1.join();
            let _ = t2.join();

            // Post-condition: The token in the state should be the "newest" one (highest number).
            // But if force_refresh overwrites get_token's result with an older token, this might fail.
            // Note: In this simplified model, we can't strictly say which one *should* win
            // if they start purely concurrently, but we can check if we end up with a lower number
            // than the max generated.

            let final_token = manager.current_token().unwrap();
            let max_generated = manager.counter.load(Ordering::SeqCst);

            // If we generated 2 tokens, the final state MUST be 2.
            // If it's 1, then the older token overwrote the newer one.
            if max_generated == 2 {
                assert_eq!(final_token, 2, "Stale token overwrote newer token!");
            }
        });
    }
}
