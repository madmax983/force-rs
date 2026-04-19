//! Runtime regression tests for Salesforce-originated apply tasks.

mod support;

use async_trait::async_trait;
use force::{
    auth::{AccessToken, Authenticator, TokenResponse},
    client::{ForceClient, builder},
    error::Result as ForceResult,
};
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use force_sync::{
    ForceSyncError, ObjectSync, PgStore, SyncEngine, SyncKey,
    {ChangeEnvelope, ChangeOperation, SourceCursor, SourceSystem},
};

#[derive(Debug, Clone)]
struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_owned(),
            instance_url: instance_url.to_owned(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: self.token.clone(),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_owned(),
            issued_at: "1704067200000".to_owned(),
            signature: "test_sig".to_owned(),
            expires_in: Some(7200),
            refresh_token: None,
        }))
    }

    async fn refresh(&self) -> ForceResult<AccessToken> {
        self.authenticate().await
    }
}

async fn test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
    builder()
        .authenticate(MockAuthenticator::new("test_token", &mock_server.uri()))
        .build()
        .await
        .unwrap_or_else(|error| panic!("unexpected client build error: {error}"))
}

fn sync_key() -> SyncKey {
    SyncKey::new("tenant", "Account", "external-1")
        .unwrap_or_else(|error| panic!("unexpected sync key construction error: {error}"))
}

async fn insert_salesforce_journal_row(
    pool: &deadpool_postgres::Pool,
) -> Result<(), ForceSyncError> {
    let store = PgStore::new(pool.clone());
    let envelope = ChangeEnvelope::new(
        sync_key(),
        SourceSystem::Salesforce,
        ChangeOperation::Upsert,
        chrono::Utc::now(),
        json!({
            "Id": "001000000000009AAA",
            "Name": "Incoming Salesforce"
        }),
    )
    .with_cursor(SourceCursor::SalesforceReplayId(77));
    let journal_id = store.append_journal(&envelope).await?;
    store.enqueue_apply_task(journal_id, 0).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn salesforce_originated_task_is_projected_locally_without_salesforce_echo()
-> Result<(), ForceSyncError> {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;

    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock_server)
        .await;

    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;
    insert_salesforce_journal_row(&pool).await?;

    let engine = SyncEngine::builder(client)
        .postgres(PgStore::new(pool.clone()))
        .object(ObjectSync::new("Account").external_id("ExternalId__c"))
        .build()?;

    assert_eq!(engine.run_apply_once().await?, 1);

    let db = pool.get().await?;
    let link = db
        .query_one(
            "select salesforce_id, last_source, tombstone
             from sync_link
             where tenant = 'tenant' and object_name = 'Account' and external_id = 'external-1'",
            &[],
        )
        .await?;
    assert_eq!(
        link.get::<_, Option<String>>(0).as_deref(),
        Some("001000000000009AAA")
    );
    assert_eq!(
        link.get::<_, Option<String>>(1).as_deref(),
        Some("salesforce")
    );
    assert!(!link.get::<_, bool>(2));

    let task = db
        .query_one(
            "select status, last_error from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task.get::<_, String>(0), "done");
    assert!(task.get::<_, Option<String>>(1).is_none());

    Ok(())
}
