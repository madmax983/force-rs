#![no_main]

use libfuzzer_sys::fuzz_target;
use force::api::rest::soql::SoqlQueryBuilder;
use force::api::composite::batch::BatchBuilder;
use force::auth::Authenticator;
use force::auth::token::{AccessToken, TokenResponse};
use force::client::builder;
use async_trait::async_trait;
use tokio::runtime::Runtime;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
struct MockAuthenticator;

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> force::error::Result<AccessToken> {
        let response = TokenResponse {
            access_token: "token".to_string(),
            instance_url: "https://test.salesforce.com".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "sig".to_string(),
            expires_in: None,
            refresh_token: None,
        };
        Ok(AccessToken::from_response(response))
    }
    async fn refresh(&self) -> force::error::Result<AccessToken> {
        self.authenticate().await
    }
}

fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().unwrap())
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let rt = get_runtime();
        rt.block_on(async {
            let auth = MockAuthenticator;
            let client = builder().authenticate(auth).build().await.expect("client");
            let builder = client.composite().batch();

            let query = SoqlQueryBuilder::new()
                .select(&[s])
                .from(s)
                .where_eq(s, s);

            // We do not catch panics so that the fuzzer reports them!
            let _ = builder.query(query);
        });
    }
});
