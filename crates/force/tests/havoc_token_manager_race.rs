use async_trait::async_trait;
use chrono::{Duration, Utc};
use force::auth::{AccessToken, Authenticator, TokenManager, TokenResponse};
use force::error::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Barrier;

#[derive(Debug)]
struct RaceAuth {
    count: AtomicUsize,
}

#[async_trait]
impl Authenticator for RaceAuth {
    async fn authenticate(&self) -> Result<AccessToken> {
        let val = self.count.fetch_add(1, Ordering::SeqCst);
        let resp = TokenResponse {
            access_token: format!("auth_{}", val),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: (Utc::now() + Duration::hours(2))
                .timestamp_millis()
                .to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(resp))
    }

    async fn refresh(&self) -> Result<AccessToken> {
        let val = self.count.fetch_add(1, Ordering::SeqCst);
        let resp = TokenResponse {
            access_token: format!("refresh_{}", val),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: (Utc::now() + Duration::hours(2))
                .timestamp_millis()
                .to_string(),
            signature: String::new(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(resp))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_havoc_token_manager_race() {
    let auth = RaceAuth {
        count: AtomicUsize::new(0),
    };
    let manager = Arc::new(TokenManager::new(auth));

    let num_tasks = 1000;
    let barrier = Arc::new(Barrier::new(num_tasks));

    let mut handles = vec![];

    for i in 0..num_tasks {
        let m = manager.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;

            let op = i % 3;
            match op {
                0 => {
                    let _ = m.token().await;
                }
                1 => {
                    let _ = m.force_refresh().await;
                }
                2 => {
                    m.clear().await;
                }
                _ => unreachable!(),
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}
