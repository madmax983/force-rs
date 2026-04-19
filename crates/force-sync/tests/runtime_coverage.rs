//! Additional runtime integration tests for coverage of DELETE, retry, and
//! permanent error code paths.
//!
//! All tests that touch the database are combined into a single function to
//! avoid schema collision when run in parallel.

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
    matchers::{header, method, path},
};

use force_sync::{
    ChangeEnvelope, ChangeOperation, ForceSyncError, ObjectSync, PgStore, SourceCursor,
    SourceSystem, SyncEngine, SyncKey,
};

// ── Shared helpers ───────────────────────────────────────────────────

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

fn sync_key(external_id: &str) -> SyncKey {
    SyncKey::new("tenant", "Account", external_id)
        .unwrap_or_else(|error| panic!("unexpected sync key construction error: {error}"))
}

/// Inserts a journal row and an apply task. Returns the `task_id`.
async fn insert_journal_and_task(
    pool: &deadpool_postgres::Pool,
    operation: ChangeOperation,
    source: SourceSystem,
    payload: serde_json::Value,
    external_id: &str,
) -> Result<i64, ForceSyncError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CURSOR_SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = CURSOR_SEQ.fetch_add(1, Ordering::SeqCst);

    let store = PgStore::new(pool.clone());
    let cursor = match source {
        SourceSystem::Postgres => SourceCursor::PostgresLsn(format!("0/{seq:08X}")),
        SourceSystem::Salesforce => SourceCursor::SalesforceReplayId(
            i64::try_from(seq).unwrap_or_else(|e| panic!("cursor seq overflow: {e}")),
        ),
    };
    let envelope = ChangeEnvelope::new(
        sync_key(external_id),
        source,
        operation,
        chrono::Utc::now(),
        payload,
    )
    .with_cursor(cursor);
    let journal_id = store.append_journal(&envelope).await?;
    let task_id = store.enqueue_apply_task(journal_id, 0).await?;
    Ok(task_id)
}

async fn insert_link(
    pool: &deadpool_postgres::Pool,
    salesforce_id: &str,
    external_id: &str,
) -> Result<(), ForceSyncError> {
    let store = PgStore::new(pool.clone());
    let link = force_sync::SyncLink {
        tenant: "tenant".to_owned(),
        object_name: "Account".to_owned(),
        external_id: external_id.to_owned(),
        salesforce_id: Some(salesforce_id.to_owned()),
        postgres_id: None,
        last_source: Some("postgres".to_owned()),
        last_source_cursor: Some("postgres-lsn:old".to_owned()),
        last_payload_hash: None,
        tombstone: false,
    };
    store.put_link(&link).await?;
    Ok(())
}

async fn get_task_status(pool: &deadpool_postgres::Pool, task_id: i64) -> (String, Option<String>) {
    let db = pool
        .get()
        .await
        .unwrap_or_else(|e| panic!("pool error: {e}"));
    let row = db
        .query_one(
            "select status, last_error from sync_task where task_id = $1",
            &[&task_id],
        )
        .await
        .unwrap_or_else(|e| panic!("task query error: {e}"));
    (row.get::<_, String>(0), row.get::<_, Option<String>>(1))
}

fn build_engine(
    client: ForceClient<MockAuthenticator>,
    pool: &deadpool_postgres::Pool,
) -> Result<SyncEngine<MockAuthenticator>, ForceSyncError> {
    SyncEngine::builder(client)
        .postgres(PgStore::new(pool.clone()))
        .object(ObjectSync::new("Account").external_id("ExternalId__c"))
        .build()
}

/// Combined integration test that exercises multiple runtime code paths:
/// 1. Empty apply batch
/// 2. DELETE with existing link (happy path)
/// 3. DELETE without existing link (fail path)
/// 4. Retryable 503 error (retry path)
/// 5. Permanent 400 error (fail path)
/// 6. DELETE with retryable error
#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
#[allow(clippy::too_many_lines)]
async fn runtime_coverage_combined_tests() -> Result<(), ForceSyncError> {
    let mock_server = MockServer::start().await;

    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    // ── Subtest 1: Empty apply batch returns zero ────────────────────
    {
        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 0);
    }

    // ── Subtest 2: DELETE with existing link succeeds ────────────────
    {
        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001000000000001AAA",
            ))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .named("delete-success")
            .mount(&mock_server)
            .await;

        insert_link(&pool, "001000000000001AAA", "del-ok-1").await?;
        let task_id = insert_journal_and_task(
            &pool,
            ChangeOperation::Delete,
            SourceSystem::Postgres,
            json!({}),
            "del-ok-1",
        )
        .await?;

        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 1);

        let (status, _error) = get_task_status(&pool, task_id).await;
        assert_eq!(status, "done");

        // Verify link is tombstoned
        let db = pool.get().await?;
        let link = db
            .query_one(
                "select tombstone from sync_link
                 where tenant = 'tenant' and object_name = 'Account' and external_id = 'del-ok-1'",
                &[],
            )
            .await?;
        assert!(link.get::<_, bool>(0), "link should be tombstoned");
    }

    // ── Subtest 3: DELETE without existing link fails task ───────────
    {
        let task_id = insert_journal_and_task(
            &pool,
            ChangeOperation::Delete,
            SourceSystem::Postgres,
            json!({}),
            "del-no-link-1",
        )
        .await?;

        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 0);

        let (status, error) = get_task_status(&pool, task_id).await;
        assert_eq!(status, "failed");
        assert!(
            error.unwrap_or_default().contains("missing Salesforce ID"),
            "expected 'missing Salesforce ID' in error"
        );
    }

    // ── Subtest 4: Retryable 503 upsert error retries task ──────────
    {
        // 503 triggers the HTTP retry logic, so the mock may be called multiple times
        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/retry-1",
            ))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
            .named("upsert-503")
            .mount(&mock_server)
            .await;

        let task_id = insert_journal_and_task(
            &pool,
            ChangeOperation::Upsert,
            SourceSystem::Postgres,
            json!({"Name": "Retry Corp"}),
            "retry-1",
        )
        .await?;

        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 0);

        let (status, error) = get_task_status(&pool, task_id).await;
        assert_eq!(
            status, "ready",
            "retryable error should set status to ready"
        );
        assert!(
            error.unwrap_or_default().contains("503"),
            "expected '503' in error"
        );
    }

    // ── Subtest 5: Permanent 400 error fails task ───────────────────
    {
        Mock::given(method("PATCH"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/ExternalId__c/perm-1",
            ))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!([{
                "errorCode": "INVALID_FIELD",
                "message": "Invalid field: BadField__c",
                "fields": ["BadField__c"]
            }])))
            .expect(1)
            .named("upsert-400")
            .mount(&mock_server)
            .await;

        let task_id = insert_journal_and_task(
            &pool,
            ChangeOperation::Upsert,
            SourceSystem::Postgres,
            json!({"BadField__c": "value"}),
            "perm-1",
        )
        .await?;

        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 0);

        let (status, error) = get_task_status(&pool, task_id).await;
        assert_eq!(status, "failed");
        assert!(error.is_some(), "permanent failure should have an error");
    }

    // ── Subtest 6: DELETE with retryable 500 error ──────────────────
    {
        // 500 triggers the HTTP retry logic, so the mock may be called multiple times
        Mock::given(method("DELETE"))
            .and(path(
                "/services/data/v60.0/sobjects/Account/001000000000002AAA",
            ))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .named("delete-500")
            .mount(&mock_server)
            .await;

        insert_link(&pool, "001000000000002AAA", "del-retry-1").await?;
        let task_id = insert_journal_and_task(
            &pool,
            ChangeOperation::Delete,
            SourceSystem::Postgres,
            json!({}),
            "del-retry-1",
        )
        .await?;

        let engine = build_engine(test_client(&mock_server).await, &pool)?;
        assert_eq!(engine.run_apply_once().await?, 0);

        let (status, _error) = get_task_status(&pool, task_id).await;
        assert_eq!(status, "ready", "retryable delete error should retry");
    }

    Ok(())
}
