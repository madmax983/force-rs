//! Havoc race condition test for timestamp resolution in `force_refresh`.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use loom::sync::{Arc, Mutex, RwLock};
    use loom::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TokenManager {
        token: RwLock<Option<Arc<usize>>>, // arc pointer, value is timestamp
        refresh_lock: Mutex<()>,
        refresh_count: AtomicUsize,
    }

    impl TokenManager {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: RwLock::new(Some(Arc::new(100))), // timestamp 100
                refresh_lock: Mutex::new(()),
                refresh_count: AtomicUsize::new(0),
            })
        }

        fn force_refresh_fixed(&self, initial_arc: Arc<usize>) -> Arc<usize> {
            let current_arc = Some(initial_arc);

            let _lock = self.refresh_lock.lock().unwrap();

            {
                let state = self.token.read().unwrap();
                if let Some(ref t) = *state {
                    if let Some(ref current) = current_arc {
                        if !Arc::ptr_eq(t, current) {
                            return t.clone();
                        }
                    } else {
                        return t.clone();
                    }
                }
            }

            self.refresh_count.fetch_add(1, Ordering::SeqCst);
            let new_token = Arc::new(100);

            {
                let mut state = self.token.write().unwrap();
                if let Some(ref current) = *state
                    && **current > *new_token
                {
                    return current.clone();
                }
                *state = Some(new_token.clone());
            }

            new_token
        }

        fn force_refresh_flawed(&self, initial_ts: usize) -> Arc<usize> {
            let current_ts = Some(initial_ts);

            let _lock = self.refresh_lock.lock().unwrap();

            {
                let state = self.token.read().unwrap();
                if let Some(ref t) = *state
                    && Some(**t) > current_ts
                {
                    return t.clone();
                }
            }

            self.refresh_count.fetch_add(1, Ordering::SeqCst);
            let new_token = Arc::new(100);

            {
                let mut state = self.token.write().unwrap();
                if let Some(ref current) = *state
                    && **current >= *new_token
                {
                    return current.clone();
                }
                *state = Some(new_token.clone());
            }

            new_token
        }
    }

    #[test]
    #[should_panic(expected = "👺 Havoc: Timestamp resolution caused a stampede!")]
    fn test_force_refresh_timestamp_stampede_flawed() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            let t1 = thread::spawn(move || m1.force_refresh_flawed(100));
            let t2 = thread::spawn(move || m2.force_refresh_flawed(100));

            let _ = t1.join();
            let _ = t2.join();

            let count = manager.refresh_count.load(Ordering::SeqCst);
            assert!(
                count < 2,
                "👺 Havoc: Timestamp resolution caused a stampede!"
            );
        });
    }

    #[test]
    fn test_force_refresh_timestamp_stampede_fixed() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            let initial_arc = manager.token.read().unwrap().as_ref().unwrap().clone();
            let arc1 = initial_arc.clone();
            let arc2 = initial_arc;

            let t1 = thread::spawn(move || m1.force_refresh_fixed(arc1));
            let t2 = thread::spawn(move || m2.force_refresh_fixed(arc2));

            let _ = t1.join();
            let _ = t2.join();

            let count = manager.refresh_count.load(Ordering::SeqCst);
            assert!(
                count < 2,
                "👺 Havoc: Timestamp resolution caused a stampede!"
            );
        });
    }
}
