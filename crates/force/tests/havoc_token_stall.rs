//! Havoc regression test for token stall during soft expiry.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use async_trait::async_trait;
use chrono::Utc;
use force::auth::authenticator::Authenticator;
use force::auth::token::{AccessToken, TokenResponse};
use force::auth::token_manager::TokenManager;
use force::error::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Instant};

// Mock Authenticator for testing TokenManager stalling
#[derive(Debug)]
struct MockAuthenticator {
    refresh_count: Arc<AtomicUsize>,
    refresh_delay_ms: u64,
}

impl MockAuthenticator {
    fn new(delay_ms: u64) -> Self {
        Self {
            refresh_count: Arc::new(AtomicUsize::new(0)),
            refresh_delay_ms: delay_ms,
        }
    }
}

fn create_token(token_value: String, expires_in_seconds: u64) -> AccessToken {
    let response = TokenResponse {
        access_token: token_value,
        instance_url: "https://test.salesforce.com".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: Utc::now().timestamp_millis().to_string(),
        signature: "sig".to_string(),
        expires_in: Some(expires_in_seconds),
        refresh_token: None,
    };
    AccessToken::from_response(response)
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> Result<AccessToken> {
        // Return a token that expires in 30 seconds
        // buffer is 60s, so it IS expired for "refresh" purposes (is_expired() == true),
        // but valid for "use" purposes (expires_at > now).
        Ok(create_token("soft_expired_token".to_string(), 30))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        self.refresh_count.fetch_add(1, Ordering::SeqCst);
        sleep(std::time::Duration::from_millis(self.refresh_delay_ms)).await;

        Ok(create_token(
            format!("refreshed_token_{}", self.refresh_count.load(Ordering::SeqCst)),
            3600,
        ))
    }
}

#[tokio::test]
async fn test_token_stall_during_soft_expiry() {
    let delay_ms = 500;
    let auth = MockAuthenticator::new(delay_ms);
    let manager = Arc::new(TokenManager::new(auth));

    // 1. Initial auth to set the "soft expired" token
    let token1 = manager.token().await.expect("Initial auth failed");
    assert_eq!(token1.as_str(), "soft_expired_token");

    // 2. Spawn concurrent tasks
    // Task A will trigger the refresh because token is "expired" (buffer check)
    let manager_a = manager.clone();
    let task_a = tokio::spawn(async move {
        let start = Instant::now();
        let token = manager_a.token().await.expect("Task A failed");
        let duration = start.elapsed();
        (token, duration)
    });

    // Short sleep to ensure Task A starts and grabs the lock/enters refresh
    sleep(std::time::Duration::from_millis(50)).await;

    // Task B should ideally return IMMEDIATELY with the old token
    let manager_b = manager.clone();
    let task_b = tokio::spawn(async move {
        let start = Instant::now();
        let token = manager_b.token().await.expect("Task B failed");
        let duration = start.elapsed();
        (token, duration)
    });

    let (token_a, duration_a) = task_a.await.expect("Task A panic");
    let (token_b, duration_b) = task_b.await.expect("Task B panic");

    println!("Task A duration: {duration_a:?}");
    println!("Task B duration: {duration_b:?}");

    // Task A should have refreshed
    assert!(token_a.as_str().starts_with("refreshed_token_"));
    assert!(duration_a.as_millis() >= u128::from(delay_ms));

    // FIX VERIFICATION: Task B should NOT be blocked
    if duration_b.as_millis() < 50 {
        println!("SUCCESS: Task B was NOT blocked! Fix verified.");
    } else {
        println!("FAILURE: Task B WAS blocked! Duration: {duration_b:?}");
        panic!("Task B was blocked");
    }

    // Check that we got the old token
    if token_b.as_str().starts_with("refreshed_token_") {
        println!("Task B got REFRESHED token (blocking behavior)");
    } else {
        println!("Task B got OLD token (non-blocking behavior)");
        assert_eq!(token_b.as_str(), "soft_expired_token");
    }
}
