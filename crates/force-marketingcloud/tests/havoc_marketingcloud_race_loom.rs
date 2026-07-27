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
    tokens: HashMap<CacheKey, Arc<String>>,
    clear_counts: HashMap<CacheKey, usize>,
}

struct TokenManagerLoom {
    cache: RwLock<CacheState>,
    locks: Mutex<HashMap<CacheKey, Arc<Mutex<()>>>>,
}

impl TokenManagerLoom {
    fn new() -> Self {
        Self {
            cache: RwLock::new(CacheState {
                tokens: HashMap::new(),
                clear_counts: HashMap::new(),
            }),
            locks: Mutex::new(HashMap::new()),
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn token(&self, gen_arc: Arc<Mutex<usize>>) -> Arc<String> {
        let key = None;
        if let Some(token) = self.cached_valid(&key) {
            return token;
        }

        let key_lock = self.key_lock(&key);
        let _guard = key_lock.lock().unwrap();

        if let Some(token) = self.cached_valid(&key) {
            return token;
        }

        let clear_count = {
            let cache = self.cache.read().unwrap();
            *cache.clear_counts.get(&key).unwrap_or(&0)
        };

        let g = *gen_arc.lock().unwrap();
        let token = Arc::new(format!("new_{g}"));

        let mut cache = self.cache.write().unwrap();
        if *cache.clear_counts.get(&key).unwrap_or(&0) == clear_count {
            cache.tokens.insert(key, token.clone());
        }
        token
    }

    #[allow(clippy::needless_pass_by_value)]
    fn invalidate(&self, gen_arc: Arc<Mutex<usize>>) {
        let key = None;
        *gen_arc.lock().unwrap() += 1;
        let mut cache = self.cache.write().unwrap();
        cache.tokens.remove(&key);
        *cache.clear_counts.entry(key).or_insert(0) += 1;
    }

    fn cached_valid(&self, key: &CacheKey) -> Option<Arc<String>> {
        self.cache.read().unwrap().tokens.get(key).cloned()
    }

    fn key_lock(&self, key: &CacheKey) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
#[test]
fn test_stale_resurrection() {
    loom::model(|| {
        let manager = Arc::new(TokenManagerLoom::new());
        let gen_arc = Arc::new(Mutex::new(0));

        let m1 = manager.clone();
        let g1 = gen_arc.clone();
        let t1 = thread::spawn(move || {
            m1.token(g1);
        });

        let m2 = manager.clone();
        let g2 = gen_arc.clone();
        let t2 = thread::spawn(move || {
            m2.invalidate(g2);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let final_gen = *gen_arc.lock().unwrap();
        if final_gen > 0 {
            if let Some(token) = manager.cache.read().unwrap().tokens.get(&None) {
                assert_ne!(
                    token.as_str(),
                    "new_0",
                    "👺 Havoc: TOCTOU Race Condition! Invalidate was overwritten by stale in-flight token!"
                );
            }
        }
    });
}
