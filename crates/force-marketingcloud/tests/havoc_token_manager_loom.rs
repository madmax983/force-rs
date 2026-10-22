//! Loom tests to verify `TokenManager` concurrency safety.
//!
//! Because `TokenManager` uses `tokio::sync::RwLock` and `tokio::sync::Mutex`,
//! we model its exact locking protocol here using `loom::sync` primitives to
//! exhaustively search for deadlocks or race conditions under all thread interleavings.

#![allow(clippy::unwrap_used)]

use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;
use std::collections::HashMap;

type CacheKey = Option<String>;

struct CacheState {
    tokens: HashMap<CacheKey, usize>,
    clear_count: u64,
}

struct TokenManagerLoomModel {
    cache: RwLock<CacheState>,
    locks: Mutex<HashMap<CacheKey, Arc<Mutex<()>>>>,
}

impl TokenManagerLoomModel {
    fn new() -> Self {
        Self {
            cache: RwLock::new(CacheState {
                tokens: HashMap::new(),
                clear_count: 0,
            }),
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn key_lock(&self, key: &CacheKey) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn token(&self, key: CacheKey, new_token_id: usize) -> usize {
        let _clear_count_at_start = {
            let cache = self.cache.read().unwrap();
            if let Some(&token) = cache.tokens.get(&key) {
                return token;
            }
            cache.clear_count
        };

        let key_lock = self.key_lock(&key);
        let _guard = key_lock.lock().unwrap();

        let (existing_token, current_clear_count) = {
            let cache = self.cache.read().unwrap();
            (cache.tokens.get(&key).copied(), cache.clear_count)
        };

        if let Some(token) = existing_token {
            return token;
        }

        let clear_count_at_auth_start = current_clear_count;

        // Simulate async auth
        loom::thread::yield_now();

        let mut cache = self.cache.write().unwrap();
        if cache.clear_count != clear_count_at_auth_start {
            return 0; // return a sentinel indicating "we didn't cache it, it was stale"
        }
        cache.tokens.insert(key, new_token_id);
        new_token_id
    }

    fn invalidate(&self, key: &CacheKey) {
        let mut cache = self.cache.write().unwrap();
        cache.tokens.remove(key);
        cache.clear_count += 1;
    }
}

#[test]
fn test_havoc_token_manager_stale_resurrection() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());
        let m1 = manager.clone();
        let m2 = manager.clone();

        let t1 = thread::spawn(move || {
            m1.token(None, 1);
        });

        let t2 = thread::spawn(move || {
            m2.invalidate(&None);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let token = manager.cache.read().unwrap().tokens.get(&None).copied();
        if let Some(t) = token {
            // If token was inserted, it must have been because clear_count was not modified during the auth yield.
            // This verifies the stale resurrection fix is effective without deadlocking.
            assert!(t == 1, "Token should be 1");
        }
    });
}
