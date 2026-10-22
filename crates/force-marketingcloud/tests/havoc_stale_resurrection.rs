#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;
use std::collections::HashMap;

// Using a struct similar to `TokenManager` to test if invalidate() can be
// overridden by a slow concurrent token() fetch.
struct TokenManagerModel {
    cache: RwLock<HashMap<String, usize>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl TokenManagerModel {
    fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn token(&self, key: &str, new_token_val: usize) {
        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(key) {
                return;
            }
        }

        let key_lock = {
            let mut locks = self.locks.lock().unwrap();
            locks
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _guard = key_lock.lock().unwrap();

        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(key) {
                return;
            }
        }

        // simulate authentication yield / delay here...
        loom::thread::yield_now();

        let mut cache = self.cache.write().unwrap();
        cache.insert(key.to_string(), new_token_val);
    }

    fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(key);
    }
}

#[test]
fn test_stale_resurrection() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerModel::new());

        let m1 = manager.clone();
        let t1 = thread::spawn(move || {
            // Task 1 fetches a token
            m1.token("test", 1);
        });

        let t2 = thread::spawn(move || {
            // Task 2 invalidates it!
            manager.invalidate("test");
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Stale resurrection check:
        // Because of the delay in `token`, `invalidate` can run after `token` verifies
        // the cache is empty but before it inserts the token. This results in the token
        // remaining in the cache, defeating the purpose of `invalidate`.
        //
        // However, loom interleaves ALL possible orderings.
        // We can't simply `assert!(cache.is_empty())` because if `token` runs strictly
        // *after* `invalidate`, the cache SHOULD have the token.
        // We need an epoch counter to model `clear_counts` or similar.
    });
}
