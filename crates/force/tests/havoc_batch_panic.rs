//! Havoc chaos test for `BatchBuilder::query` panics.
//!
//! This test verifies the vulnerability where passing an invalid `SoqlQueryBuilder`
//! to `BatchBuilder::query` causes a hard panic instead of returning a `Result::Err`.

#[cfg(test)]
mod tests {
    use force::client::builder;
    use force::auth::client_credentials::ClientCredentials;
    use force::api::rest::soql::SoqlQueryBuilder;

    #[tokio::test]
    #[should_panic(expected = "Invalid query builder")]
    async fn test_batch_query_panic() {
        let auth = ClientCredentials::new_sandbox("client_id", "client_secret");
        let client = builder().authenticate(auth).build().await.unwrap();

        let batch_builder = client.composite().batch();

        // This should cause a validation error because no `from` was specified
        let invalid_query = SoqlQueryBuilder::new().select(&["Id"]);

        // This will panic internally!
        let _ = batch_builder.query(invalid_query);
    }
}
