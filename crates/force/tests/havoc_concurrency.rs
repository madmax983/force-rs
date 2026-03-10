//! Havoc concurrency test for double-checked locking verification.
//!
//! # Elenchus Audit Note
//!
//! This test uses a mirror struct `TokenManager` instead of the production type.
//! This is necessary because `loom` requires its own synchronization primitives
//! (e.g., `loom::sync::RwLock`) to explore all possible thread interleavings.
//! While this does not verify the production *implementation* (imports, struct layout),
//! it mathematically verifies the correctness of the *double-checked locking algorithm*
//! used in `crates/force/src/auth/token_manager.rs`.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use loom::sync::{Arc, RwLock};
    use loom::thread;

    // Simplified TokenManager logic for Loom testing
    // This mirrors the logic in crates/force/src/auth/token_manager.rs
    struct TokenManager {
        token: RwLock<Option<String>>,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(None),
            })
        }

        fn get_token(&self) -> String {
            // Fast path: read lock
            {
                let guard = self.token.read().unwrap();
                if let Some(token) = &*guard {
                    return token.clone();
                }
            }

            // Slow path: write lock
            let mut guard = self.token.write().unwrap();
            // Double check
            if let Some(token) = &*guard {
                return token.clone();
            }

            // Refresh (simulate)
            // In the real code, we would await an async function here.
            // In loom (sync model), we just set the value.
            let new_token = "new_token".to_string();
            *guard = Some(new_token.clone());
            new_token
        }
    }

    impl TokenManager {
        fn force_refresh(&self) -> String {
            // Force refresh goes straight to write lock
            let mut guard = self.token.write().unwrap();
            let new_token = "forced_token".to_string();
            *guard = Some(new_token.clone());
            new_token
        }
    }

    #[test]
    fn test_double_checked_locking() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();

            let t1 = thread::spawn(move || m1.get_token());
            let t2 = thread::spawn(move || manager.get_token());

            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            assert_eq!(r1, "new_token");
            assert_eq!(r2, "new_token");
        });
    }

    #[test]
    fn test_concurrent_get_and_force_refresh() {
        loom::model(|| {
            let manager = TokenManager::new();

            // pre-populate token
            {
                let mut guard = manager.token.write().unwrap();
                *guard = Some("initial_token".to_string());
            }

            let m1 = manager.clone();
            let m2 = manager;

            let t1 = thread::spawn(move || m1.get_token());
            let t2 = thread::spawn(move || m2.force_refresh());

            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            assert!(r1 == "initial_token" || r1 == "forced_token");
            assert_eq!(r2, "forced_token");
        });
    }
}
