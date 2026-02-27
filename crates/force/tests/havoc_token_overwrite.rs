//! Havoc race condition test for `TokenManager::force_refresh`.
//!
//! This test simulates the race condition where `force_refresh` (which refreshes outside the lock)
//! competes with `get_token` (which refreshes inside the lock).

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
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
            // Loom will explore context switches here
            self.counter.fetch_add(1, Ordering::SeqCst) + 1
        }

        // Simulates TokenManager::get_token_arc (the slow path part)
        // BUG: This method blindly overwrites the token without checking timestamps
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

        // Simulates TokenManager::force_refresh
        // This method refreshes OUTSIDE the lock, then acquires write lock
        fn force_refresh(&self) -> usize {
            // Refresh OUTSIDE lock (simulate network latency)
            let new_token = self.generate_token();

            // Update state INSIDE lock
            {
                let mut guard = self.token.write().unwrap();

                // Even if force_refresh checks (which it does in the real code),
                // the issue is that get_token does NOT check.
                // So if force_refresh wins the race to update, get_token might overwrite it.

                if let Some(current) = *guard
                    && current > new_token
                {
                    return current;
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
    fn test_token_overwrite_race_condition() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            // Thread 1: Calls force_refresh (e.g. triggered by 401)
            let t1 = thread::spawn(move || m1.force_refresh());

            // Thread 2: Calls get_token (e.g. normal request needing token)
            let t2 = thread::spawn(move || m2.get_token());

            let _ = t1.join();
            let _ = t2.join();

            let final_token = manager.current_token().unwrap();
            let max_generated = manager.counter.load(Ordering::SeqCst);

            // If we generated 2 tokens, the final state MUST be 2 (the newest one).
            // If it's 1, then the older token (likely from get_token running slower) overwrote the newer one.
            if max_generated == 2 {
                assert_eq!(
                    final_token, 2,
                    "Stale token overwrote newer token! (Race Condition Triggered)"
                );
            }
        });
    }
}
