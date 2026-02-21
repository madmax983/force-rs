// Only compile this test when loom is enabled, or just run it with cargo test
// But loom requires specific flags usually.
// However, loom exports a `model` function that runs the closure many times.

// We need to import loom.
// Since we added loom as a dev-dependency, it's available.

#[cfg(test)]
mod tests {
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

    #[test]
    fn test_double_checked_locking() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            let t1 = thread::spawn(move || m1.get_token());
            let t2 = thread::spawn(move || m2.get_token());

            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            assert_eq!(r1, "new_token");
            assert_eq!(r2, "new_token");
        });
    }
}
