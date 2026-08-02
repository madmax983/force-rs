#[cfg(test)]
mod tests {
    use loom::sync::{Arc, RwLock};
    use loom::thread;
    use std::collections::HashMap;

    type CacheKey = Option<String>;

    struct TokenManagerModel {
        cache: RwLock<HashMap<CacheKey, Arc<String>>>,
    }

    impl TokenManagerModel {
        fn new() -> Self {
            Self {
                cache: RwLock::new(HashMap::new()),
            }
        }

        fn invalidate(&self, key: CacheKey, token_to_invalidate: Arc<String>) {
            let mut cache = self.cache.write().unwrap();
            if let Some(cached) = cache.get(&key) {
                if Arc::ptr_eq(cached, &token_to_invalidate) {
                    cache.remove(&key);
                }
            }
        }
    }

    #[test]
    fn test_marketingcloud_invalidate_race() {
        loom::model(|| {
            let manager = Arc::new(TokenManagerModel::new());
            let old_token = Arc::new("old".to_string());
            manager
                .cache
                .write()
                .unwrap()
                .insert(None, old_token.clone());

            let mut threads = vec![];

            let m1 = manager.clone();
            let token_to_invalidate = old_token.clone();
            threads.push(thread::spawn(move || {
                m1.invalidate(None, token_to_invalidate);
            }));

            let m2 = manager.clone();
            threads.push(thread::spawn(move || {
                let new_token = Arc::new("new".to_string());
                m2.cache.write().unwrap().insert(None, new_token.clone());
            }));

            for t in threads {
                t.join().unwrap();
            }

            let cache = manager.cache.read().unwrap();
            assert!(
                cache.contains_key(&None),
                "Cache is empty! We lost the new token!"
            );
        });
    }
}
