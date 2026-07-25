//! Proactive, per-business-unit token management.
//!
//! Because Marketing Cloud tokens are short-lived and carry no refresh token,
//! "refresh" is simply re-authentication. Tokens are cached per business unit
//! (MID) so that calls targeting different units each keep their own token, and
//! a per-key single-flight lock prevents a stampede of concurrent auth requests.

use crate::auth::credentials::Authenticator;
use crate::auth::token::AccessToken;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Cache key: the effective per-call account id override (`None` = builder default).
type CacheKey = Option<String>;

/// Thread-safe, per-MID token cache with proactive re-authentication.
#[derive(Debug)]
struct CacheState {
    tokens: HashMap<CacheKey, Arc<AccessToken>>,
    clear_counts: HashMap<CacheKey, u64>,
}

/// Thread-safe, per-MID token cache with proactive re-authentication.
#[derive(Debug)]
pub struct TokenManager {
    /// The underlying authenticator.
    authenticator: Arc<dyn Authenticator>,

    /// Cached tokens keyed by business-unit override.
    cache: RwLock<CacheState>,

    /// Per-key single-flight locks serializing concurrent refreshes.
    locks: Mutex<HashMap<CacheKey, Arc<Mutex<()>>>>,
}

impl TokenManager {
    /// Creates a new token manager wrapping the given authenticator.
    #[must_use]
    pub fn new(authenticator: Arc<dyn Authenticator>) -> Self {
        Self {
            authenticator,
            cache: RwLock::new(CacheState {
                tokens: HashMap::new(),
                clear_counts: HashMap::new(),
            }),
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Returns a valid token for the given business unit, authenticating if needed.
    ///
    /// A cached token is reused unless it is missing or within its soft-expiry
    /// window, in which case it is proactively re-fetched under a single-flight
    /// lock so concurrent callers make at most one auth request per key.
    ///
    /// # Errors
    ///
    /// Returns an error if authentication fails.
    pub async fn token(&self, account_id: Option<&str>) -> Result<Arc<AccessToken>> {
        let key: CacheKey = account_id.map(ToString::to_string);

        if let Some(token) = self.cached_valid(&key).await {
            return Ok(token);
        }

        let key_lock = self.key_lock(&key).await;
        let _guard = key_lock.lock().await;

        // Double-check: another task may have refreshed while we waited.
        if let Some(token) = self.cached_valid(&key).await {
            return Ok(token);
        }

        let pre_auth_count = self
            .cache
            .read()
            .await
            .clear_counts
            .get(&key)
            .copied()
            .unwrap_or(0);
        let token = Arc::new(self.authenticator.authenticate(account_id).await?);

        let mut cache = self.cache.write().await;
        let post_auth_count = cache.clear_counts.get(&key).copied().unwrap_or(0);
        if pre_auth_count == post_auth_count {
            cache.tokens.insert(key, token.clone());
        }
        drop(cache);

        Ok(token)
    }

    /// Removes any cached token for the given business unit.
    ///
    /// Useful for handling a `401` where the server invalidated the token early.
    pub async fn invalidate(&self, account_id: Option<&str>) {
        let key: CacheKey = account_id.map(ToString::to_string);
        let mut cache = self.cache.write().await;
        cache.tokens.remove(&key);
        *cache.clear_counts.entry(key).or_insert(0) += 1;
    }

    /// Returns the cached token for `key` if present and not due for refresh.
    async fn cached_valid(&self, key: &CacheKey) -> Option<Arc<AccessToken>> {
        let cache = self.cache.read().await;
        cache
            .tokens
            .get(key)
            .filter(|t| !t.needs_refresh())
            .cloned()
    }

    /// Fetches (creating if necessary) the single-flight lock for `key`.
    async fn key_lock(&self, key: &CacheKey) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingAuth {
        calls: AtomicUsize,
        lifetime: Duration,
    }

    impl CountingAuth {
        fn new(lifetime: Duration) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                lifetime,
            }
        }
    }

    #[async_trait]
    impl Authenticator for CountingAuth {
        async fn authenticate(&self, account_id: Option<&str>) -> Result<AccessToken> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let label = account_id.unwrap_or("default");
            AccessToken::new_for_test(
                &format!("{label}-token-{n}"),
                "https://sub.rest.marketingcloudapis.com/",
                Utc::now() + self.lifetime,
            )
        }
    }

    #[tokio::test]
    async fn caches_token_across_calls() {
        let auth = Arc::new(CountingAuth::new(Duration::hours(1)));
        let manager = TokenManager::new(auth.clone());

        let t1 = manager.token(None).await.unwrap();
        let t2 = manager.token(None).await.unwrap();
        assert_eq!(t1.as_str(), t2.as_str());
        assert_eq!(auth.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reauthenticates_when_token_needs_refresh() {
        // Soft-expired immediately: lifetime under the 60s buffer.
        let auth = Arc::new(CountingAuth::new(Duration::seconds(30)));
        let manager = TokenManager::new(auth.clone());

        let _ = manager.token(None).await.unwrap();
        let _ = manager.token(None).await.unwrap();
        assert_eq!(auth.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn distinct_business_units_get_distinct_tokens() {
        let auth = Arc::new(CountingAuth::new(Duration::hours(1)));
        let manager = TokenManager::new(auth.clone());

        let default = manager.token(None).await.unwrap();
        let bu = manager.token(Some("999")).await.unwrap();

        assert_ne!(default.as_str(), bu.as_str());
        assert!(bu.as_str().contains("999"));
        assert_eq!(auth.calls.load(Ordering::SeqCst), 2);

        // Each key is independently cached.
        let _ = manager.token(Some("999")).await.unwrap();
        assert_eq!(auth.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn invalidate_forces_reauthentication() {
        let auth = Arc::new(CountingAuth::new(Duration::hours(1)));
        let manager = TokenManager::new(auth.clone());

        let _ = manager.token(None).await.unwrap();
        manager.invalidate(None).await;
        let _ = manager.token(None).await.unwrap();
        assert_eq!(auth.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn havoc_race_condition_stale_token_resurrection() {
        #[derive(Debug)]
        struct HavocAuth {
            b1: Arc<tokio::sync::Barrier>,
            b2: Arc<tokio::sync::Barrier>,
            calls: AtomicUsize,
        }

        #[async_trait]
        impl Authenticator for HavocAuth {
            async fn authenticate(&self, _account_id: Option<&str>) -> Result<AccessToken> {
                let call = self.calls.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    self.b1.wait().await;
                    self.b2.wait().await;
                    AccessToken::new_for_test(
                        "stale-token",
                        "https://stale/",
                        Utc::now() + Duration::hours(1),
                    )
                } else {
                    AccessToken::new_for_test(
                        "fresh-token",
                        "https://fresh/",
                        Utc::now() + Duration::hours(1),
                    )
                }
            }
        }

        let b1 = Arc::new(tokio::sync::Barrier::new(2));
        let b2 = Arc::new(tokio::sync::Barrier::new(2));

        let auth = Arc::new(HavocAuth {
            b1: b1.clone(),
            b2: b2.clone(),
            calls: AtomicUsize::new(0),
        });

        let manager = Arc::new(TokenManager::new(auth));

        let m_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let Ok(token) = m_clone.token(None).await else {
                panic!("Expected Ok")
            };
            token
        });

        b1.wait().await; // Wait for in-flight auth to start
        manager.invalidate(None).await; // The Trigger: invalidate while in-flight
        b2.wait().await; // Let auth finish

        let Ok(_) = handle.await else {
            panic!("Expected Ok")
        };

        let Ok(final_token) = manager.token(None).await else {
            panic!("Expected Ok")
        };

        assert_eq!(
            final_token.as_str(),
            "fresh-token",
            "Havoc: TOCTOU Race condition detected! Invalidation was lost and stale token resurrected!"
        );
    }

    #[tokio::test]
    async fn concurrent_requests_single_flight() {
        let auth = Arc::new(CountingAuth::new(Duration::hours(1)));
        let manager = Arc::new(TokenManager::new(auth.clone()));

        let mut handles = Vec::new();
        for _ in 0..25 {
            let m = manager.clone();
            handles.push(tokio::spawn(async move { m.token(None).await.unwrap() }));
        }
        for handle in handles {
            let _ = handle.await.unwrap();
        }
        // Single-flight: exactly one auth despite concurrency.
        assert_eq!(auth.calls.load(Ordering::SeqCst), 1);
    }
}
