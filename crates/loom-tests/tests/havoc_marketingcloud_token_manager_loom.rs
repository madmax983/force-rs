//! Loom tests to verify `TokenManager` concurrency safety.
//!
//! Because `TokenManager` uses `tokio::sync::RwLock` and `tokio::sync::Mutex`,
//! we model its exact locking protocol here using `loom::sync` primitives to
//! exhaustively search for deadlocks or race conditions under all thread interleavings.
//! This test is standalone and uses `std::collections::HashMap` to mock the behavior.

#![allow(clippy::unwrap_used)]

use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;
use std::collections::HashMap;

type CacheKey = Option<String>;

struct TokenManagerLoomModel {
    cache: RwLock<HashMap<CacheKey, usize>>,
    locks: Mutex<HashMap<CacheKey, Arc<Mutex<()>>>>,
}

impl TokenManagerLoomModel {
    fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn token(&self, account_id: Option<&str>) -> usize {
        let key: CacheKey = account_id.map(ToString::to_string);

        if let Some(&token) = self.cache.read().unwrap().get(&key) {
            return token;
        }

        let key_lock = {
            let mut locks = self.locks.lock().unwrap();
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = key_lock.lock().unwrap();

        if let Some(&token) = self.cache.read().unwrap().get(&key) {
            return token;
        }

        let token = 1; // Simulated new token
        self.cache.write().unwrap().insert(key, token);
        token
    }

    #[allow(dead_code)]
    fn invalidate(&self, account_id: Option<&str>) {
        let key: CacheKey = account_id.map(ToString::to_string);
        self.cache.write().unwrap().remove(&key);
    }
}

#[test]
fn test_havoc_marketingcloud_token_manager_loom() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());
        let mut threads = vec![];

        let m1 = manager.clone();
        threads.push(thread::spawn(move || {
            m1.token(None);
        }));

        let m2 = manager.clone();
        threads.push(thread::spawn(move || {
            m2.token(None);
        }));

        for t in threads {
            t.join().unwrap();
        }
    });
}
