//! End-to-end Postgres outbox to Salesforce REST runtime test.

mod support;

use async_trait::async_trait;
use force::{
    auth::{AccessToken, Authenticator, TokenResponse},
    client::{ForceClient, builder},
    error::Result as ForceResult,
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

use force_sync::{ForceSyncError, ObjectSync, PgStore, SyncEngine};

#[derive(Debug, Clone)]
struct MockAuthenticator {
    token: String,
    instance_url: String,
}

impl MockAuthenticator {
    fn new(token: &str, instance_url: &str) -> Self {
        Self {
            token: token.to_string(),
            instance_url: instance_url.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self) -> ForceResult<AccessToken> {
        Ok(AccessToken::from_response(TokenResponse {
            access_token: secrecy::SecretString::new(self.token.clone().into()),
            instance_url: self.instance_url.clone(),
            token_type: "Bearer".to_string(),
            issued_at: "1704067200000".to_string(),
            signature: "test_sig".to_string(),
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

async fn insert_outbox_row(
    pool: &deadpool_postgres::Pool,
    source_cursor: &str,
) -> Result<(), ForceSyncError> {
    let client = pool.get().await?;
    let payload = json!({"Name": "Acme Corp"});
    client
        .execute(
            "insert into force_sync_outbox (
                tenant,
                object_name,
                external_id,
                source_cursor,
                op,
                tombstone,
                payload
            ) values (
                'tenant',
                'Account',
                'external-1',
                $2,
                'upsert',
                false,
                $1::jsonb
            )",
            &[&payload, &source_cursor],
        )
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn run_capture_and_apply_once_converges_one_postgres_record() -> Result<(), ForceSyncError> {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v67.0/sobjects/Account/ExternalId__c/external-1",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({"Name": "Acme Corp"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "001000000000001AAA",
            "success": true,
            "created": true,
            "errors": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;
    insert_outbox_row(&pool, "postgres-lsn-1").await?;

    let store = PgStore::new(pool.clone());
    let engine = SyncEngine::builder(client)
        .postgres(store)
        .object(ObjectSync::new("Account").external_id("ExternalId__c"))
        .build()?;

    assert_eq!(engine.run_capture_postgres_once().await?, 1);
    assert_eq!(engine.run_apply_once().await?, 1);

    let db = pool.get().await?;
    let link = db
        .query_one(
            "select salesforce_id, last_source, last_source_cursor, tombstone
             from sync_link
             where tenant = 'tenant' and object_name = 'Account' and external_id = 'external-1'",
            &[],
        )
        .await?;
    assert_eq!(
        link.get::<_, Option<String>>(0).as_deref(),
        Some("001000000000001AAA")
    );
    assert_eq!(
        link.get::<_, Option<String>>(1).as_deref(),
        Some("postgres")
    );
    assert_eq!(
        link.get::<_, Option<String>>(2).as_deref(),
        Some("postgres-lsn:postgres-lsn-1")
    );
    assert!(!link.get::<_, bool>(3));

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

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn replayed_postgres_change_is_not_reapplied_to_salesforce() -> Result<(), ForceSyncError> {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;

    Mock::given(method("PATCH"))
        .and(path(
            "/services/data/v67.0/sobjects/Account/ExternalId__c/external-1",
        ))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({"Name": "Acme Corp"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "001000000000001AAA",
            "success": true,
            "created": true,
            "errors": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    insert_outbox_row(&pool, "postgres-lsn-1").await?;
    let engine = SyncEngine::builder(client)
        .postgres(PgStore::new(pool.clone()))
        .object(ObjectSync::new("Account").external_id("ExternalId__c"))
        .build()?;

    assert_eq!(engine.run_capture_postgres_once().await?, 1);
    assert_eq!(engine.run_apply_once().await?, 1);

    insert_outbox_row(&pool, "postgres-lsn-2").await?;
    assert_eq!(engine.run_capture_postgres_once().await?, 1);
    assert_eq!(engine.run_apply_once().await?, 0);

    let db = pool.get().await?;
    let task = db
        .query_one(
            "select status, last_error from sync_task where task_kind = 'apply' order by task_id desc limit 1",
            &[],
        )
        .await?;
    assert_eq!(task.get::<_, String>(0), "done");
    assert!(task.get::<_, Option<String>>(1).is_none());

    Ok(())
}
