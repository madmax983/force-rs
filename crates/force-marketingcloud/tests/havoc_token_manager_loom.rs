//! Loom tests to verify `TokenManager` concurrency safety in `force-marketingcloud`.

#![allow(clippy::unwrap_used)]

use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;
use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
struct TokenState {
    token_id: usize,
}

type CacheKey = Option<String>;

struct TokenManagerLoomModel {
    cache: RwLock<HashMap<CacheKey, Arc<TokenState>>>,
    locks: Mutex<HashMap<CacheKey, Arc<Mutex<()>>>>,
}

impl TokenManagerLoomModel {
    fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn cached_valid(&self, key: &CacheKey) -> Option<Arc<TokenState>> {
        let cache = self.cache.read().unwrap();
        cache.get(key).cloned()
    }

    fn key_lock(&self, key: &CacheKey) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn token(&self, account_id: Option<&str>, mock_token_id: usize) -> Arc<TokenState> {
        let key: CacheKey = account_id.map(ToString::to_string);

        if let Some(token) = self.cached_valid(&key) {
            return token;
        }

        let key_lock = self.key_lock(&key);
        let _guard = key_lock.lock().unwrap();

        if let Some(token) = self.cached_valid(&key) {
            return token;
        }

        loom::thread::yield_now();

        let token = Arc::new(TokenState {
            token_id: mock_token_id,
        });
        self.cache.write().unwrap().insert(key, token.clone());

        token
    }

    // Fixed invalidate logic
    fn invalidate_fixed(&self, account_id: Option<&str>, old_token: &Arc<TokenState>) {
        let key: CacheKey = account_id.map(ToString::to_string);
        let mut cache = self.cache.write().unwrap();
        if let Some(token) = cache.get(&key) {
            // ONLY remove if the cache still holds the exact token we tried to use and failed
            if token.token_id == old_token.token_id {
                cache.remove(&key);
            }
        }
    }
}

#[test]
fn test_havoc_marketingcloud_token_manager_loom_fixed() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());

        // Start with an old invalid token.
        let old_token = Arc::new(TokenState { token_id: 0 });
        manager
            .cache
            .write()
            .unwrap()
            .insert(None, old_token.clone());

        // Thread 1: gets a 401 using Token 0. Invalidates, then fetches Token 1.
        let m1 = manager.clone();
        let old_t1 = old_token.clone();
        let t1 = thread::spawn(move || {
            m1.invalidate_fixed(None, &old_t1);
            m1.token(None, 1)
        });

        // Thread 2: ALSO gets a 401 using Token 0. Invalidates, then does NOT fetch immediately (or fetches later).
        let m2 = manager.clone();
        let old_t2 = old_token;
        let t2 = thread::spawn(move || {
            m2.invalidate_fixed(None, &old_t2);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let final_token = manager.cached_valid(&None);

        assert!(
            final_token.is_some(),
            "👺 Havoc: `invalidate` wiped out a newly refreshed token!"
        );
    });
}
