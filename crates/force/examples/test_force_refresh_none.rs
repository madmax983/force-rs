#[tokio::main]
async fn main() {
    use force::auth::Authenticator;
    use force::auth::{AccessToken, TokenManager, TokenResponse};

    #[derive(Debug)]
    struct DummyAuth;
    #[async_trait::async_trait]
    impl Authenticator for DummyAuth {
        async fn authenticate(&self) -> force::error::Result<AccessToken> {
            Ok(AccessToken::from_response(TokenResponse {
                access_token: "dummy".into(),
                instance_url: "dummy".into(),
                token_type: "Bearer".into(),
                issued_at: "0".into(),
                signature: "".into(),
                expires_in: None,
                refresh_token: None,
            }))
        }
        async fn refresh(&self) -> force::error::Result<AccessToken> {
            self.authenticate().await
        }
    }

    let manager = TokenManager::new(DummyAuth);
    let res = manager.force_refresh().await;
    println!("{:?}", res);
}
