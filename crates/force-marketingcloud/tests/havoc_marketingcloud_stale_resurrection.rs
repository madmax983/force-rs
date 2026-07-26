#![allow(missing_docs, clippy::unwrap_used, clippy::needless_pass_by_value, clippy::redundant_clone, clippy::ref_option)]
use loom::sync::{Arc, Mutex, RwLock};
use loom::thread;
use std::collections::HashMap;

struct CacheState {
    token: Option<usize>,
    clear_count: u64,
}

struct TokenManagerLoomModel {
    cache: RwLock<HashMap<Option<String>, CacheState>>,
    locks: Mutex<HashMap<Option<String>, Arc<Mutex<()>>>>,
}

impl TokenManagerLoomModel {
    fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn key_lock(&self, key: &Option<String>) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn token(&self, key: Option<String>, fetch: impl FnOnce() -> usize) -> usize {
        {
            let cache = self.cache.read().unwrap();
            if let Some(state) = cache.get(&key) {
                if let Some(val) = state.token {
                    return val;
                }
            }
        }

        let lock = self.key_lock(&key);
        let _guard = lock.lock().unwrap();

        let clear_count = {
            let cache = self.cache.read().unwrap();
            if let Some(state) = cache.get(&key) {
                if let Some(val) = state.token {
                    return val;
                }
                state.clear_count
            } else {
                0
            }
        };

        let token_val = fetch();

        let mut cache = self.cache.write().unwrap();
        let entry = cache.entry(key).or_insert_with(|| CacheState { token: None, clear_count: 0 });
        if entry.clear_count == clear_count {
            entry.token = Some(token_val);
        }
        token_val
    }

    fn invalidate(&self, key: Option<String>) {
        let mut cache = self.cache.write().unwrap();
        let entry = cache.entry(key).or_insert_with(|| CacheState { token: None, clear_count: 0 });
        entry.token = None;
        entry.clear_count += 1;
    }
}

#[test]
fn test_havoc_marketingcloud_stale_resurrection() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoomModel::new());
        let key = Some("BU1".to_string());

        let fetch_started = Arc::new(loom::sync::atomic::AtomicBool::new(false));
        let invalidate_called = Arc::new(loom::sync::atomic::AtomicBool::new(false));

        let m1 = manager.clone();
        let k1 = key.clone();
        let fs1 = fetch_started.clone();
        let t1 = thread::spawn(move || {
            m1.token(k1, || {
                fs1.store(true, loom::sync::atomic::Ordering::SeqCst);
                loom::thread::yield_now();
                1
            });
        });

        let m2 = manager.clone();
        let k2 = key.clone();
        let fs2 = fetch_started.clone();
        let ic2 = invalidate_called.clone();
        let t2 = thread::spawn(move || {
            if fs2.load(loom::sync::atomic::Ordering::SeqCst) {
                m2.invalidate(k2);
                ic2.store(true, loom::sync::atomic::Ordering::SeqCst);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // If invalidate was called AFTER fetch started, then the cached token should be empty
        // wait, the problem is it WON'T be empty!
        // We WANT it to crash/fail to prove the bug exists.
        if invalidate_called.load(loom::sync::atomic::Ordering::SeqCst) {
            let cache = manager.cache.read().unwrap();
            let has_token = cache.get(&key).and_then(|s| s.token).is_some();
            assert!(
                !has_token,
                "👺 Havoc: Stale data resurrection detected! Token was cached after it was invalidated!"
            );
        }
    });
}
