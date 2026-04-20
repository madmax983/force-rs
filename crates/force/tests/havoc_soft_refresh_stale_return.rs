#![allow(clippy::expect_used)]
//! Havoc race condition test for stale soft-expired token returns.
//!
//! This mirrors the soft-expired branch in `TokenManager::get_token_arc`.
//! We use `loom` primitives so the scheduler can force the interleaving where:
//! 1. a reader snapshots the old soft-expired token,
//! 2. another task publishes a fresh token while still holding `refresh_lock`,
//! 3. the reader misses the lock and returns stale data instead of the published token.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use loom::sync::atomic::{AtomicBool, Ordering};
    use loom::sync::{Arc, Mutex, RwLock};
    use loom::thread;

    struct TokenManager {
        token: RwLock<usize>,
        published: AtomicBool,
        refresh_lock: Mutex<()>,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(1),
                published: AtomicBool::new(false),
                refresh_lock: Mutex::new(()),
            })
        }

        fn refresh_soft_expired(&self) -> usize {
            let _lock = self.refresh_lock.lock().expect("test failed");
            let new_token = 2;
            *self.token.write().expect("test failed") = new_token;
            self.published.store(true, Ordering::SeqCst);

            // Keep the refresh lock held after publishing the new token so loom can
            // schedule a reader into the stale-return path.
            thread::yield_now();

            new_token
        }

        fn get_soft_expired_token(&self) -> (usize, bool) {
            let snapshot = *self.token.read().expect("test failed");

            // Give the refresher a chance to publish a new token after we snapshot.
            thread::yield_now();

            self.refresh_lock.try_lock().map_or_else(
                |_| {
                    let saw_published_token = self.published.load(Ordering::SeqCst);
                    let returned_token = if saw_published_token {
                        self.current_token().max(snapshot)
                    } else {
                        snapshot
                    };
                    (returned_token, saw_published_token)
                },
                |_lock| {
                    let new_token = 3;
                    *self.token.write().expect("test failed") = new_token;
                    (new_token, false)
                },
            )
        }

        fn current_token(&self) -> usize {
            *self.token.read().expect("test failed")
        }
    }

    #[test]
    fn test_soft_refresh_try_lock_failure_returns_latest_published_token() {
        loom::model(|| {
            let manager = TokenManager::new();
            let refresher = manager.clone();
            let reader = manager;

            let refresh_task = thread::spawn(move || refresher.refresh_soft_expired());
            let reader_task = thread::spawn(move || reader.get_soft_expired_token());

            let _ = refresh_task.join().expect("test failed");
            let (returned_token, saw_published_token) = reader_task.join().expect("test failed");

            if saw_published_token {
                assert_eq!(
                    returned_token, 2,
                    "soft-expired reader returned a stale token after refresh published state"
                );
            }
        });
    }
}
