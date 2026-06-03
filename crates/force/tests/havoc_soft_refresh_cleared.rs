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
                token: RwLock::new(Some(1)),
                clear_count: RwLock::new(0),
                refresh_lock: Mutex::new(()),
            })
        }

        fn handle_soft_refresh(&self, valid_token: usize) -> Result<usize, ()> {
            let Ok(_lock) = self.refresh_lock.try_lock() else {
                return Ok(self.latest_token_or(valid_token));
            };

            thread::yield_now();
            let new_token = 2;

            // In real code `refresh_result = self.authenticator.refresh().await` happens here.
            // When it fails (Err), we hit `latest_token_or`. Let's simulate Err!
            Ok(self.latest_token_or(valid_token))
        }

        fn latest_token_or(&self, fallback: usize) -> usize {
            let current = *self.token.read().unwrap();
            match current {
                Some(current) if current >= fallback => current,
                _ => fallback,
            }
        }

        fn clear(&self) {
            *self.token.write().unwrap() = None;
            *self.clear_count.write().unwrap() += 1;
        }
    }

    #[test]
    fn test_soft_refresh_returns_cleared_token() {
        loom::model(|| {
            let manager = TokenManager::new();
            let m1 = manager.clone();
            let m2 = manager.clone();

            let t1 = thread::spawn(move || {
                m1.handle_soft_refresh(1)
            });

            let t2 = thread::spawn(move || {
                m2.clear();
                let _lock = m2.refresh_lock.lock().unwrap();
                thread::yield_now();
            });

            let r1 = t1.join().unwrap();
            let _ = t2.join().unwrap();

            let final_token = *manager.token.read().unwrap();

            if final_token.is_none() {
                if let Ok(1) = r1 {
                    panic!("👺 Havoc: Resurrected a cleared token!");
                }
            }
        });
    }
}
