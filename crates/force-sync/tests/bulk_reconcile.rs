//! Bulk apply and reconcile tests.

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
    matchers::{body_json, body_string, header, method, path},
};

use force_sync::{
    ApplyLane, ChangeEnvelope, ChangeOperation, ForceSyncError, ObjectSync, PgStore,
    PlannerContext, SalesforceApplier, SourceCursor, SourceSystem, SyncKey, SyncLink, detect_drift,
    enqueue_repair, plan_change, run_reconcile_once,
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
            access_token: secrecy::SecretString::new(self.token.clone().into()),
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

fn envelope(
    external_id: &str,
    payload: serde_json::Value,
    source: SourceSystem,
    operation: ChangeOperation,
    cursor: SourceCursor,
) -> ChangeEnvelope {
    ChangeEnvelope::new(
        sync_key(external_id),
        source,
        operation,
        chrono::Utc::now(),
        payload,
    )
    .with_cursor(cursor)
}

#[derive(Debug, Clone, serde::Serialize)]
struct BulkAccountRow {
    #[serde(rename = "External_Id__c")]
    external_id: String,
    #[serde(rename = "Name")]
    name: String,
}

#[test]
fn planner_sends_large_homogeneous_batches_to_bulk_lane() {
    let object = ObjectSync::new("Account").external_id("External_Id__c");
    let context = PlannerContext {
        object: &object,
        current_payload: None,
        batch_size: 500,
        urgent: false,
        has_dependencies: false,
    };
    let envelope = envelope(
        "external-1",
        json!({"Name": "Acme"}),
        SourceSystem::Salesforce,
        ChangeOperation::Upsert,
        SourceCursor::SalesforceReplayId(1),
    );

    let decision = plan_change(&context, &envelope);

    assert_eq!(decision.lane, ApplyLane::Bulk);
}

#[tokio::test]
async fn bulk_upsert_uses_the_external_id_field() {
    let mock_server = MockServer::start().await;
    let client = test_client(&mock_server).await;
    let applier = SalesforceApplier::new(client);

    Mock::given(method("POST"))
        .and(path("/services/data/v60.0/jobs/ingest"))
        .and(header("Authorization", "Bearer test_token"))
        .and(body_json(json!({
            "object": "Account",
            "operation": "upsert",
            "externalIdFieldName": "External_Id__c"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "750xx0000000001AAA",
            "state": "Open",
            "operation": "upsert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "externalIdFieldName": "External_Id__c"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("PUT"))
        .and(path(
            "/services/data/v60.0/jobs/ingest/750xx0000000001AAA/batches",
        ))
        .and(body_string(
            "External_Id__c,Name\nexternal-1,Acme\nexternal-2,Acme 2\n",
        ))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("PATCH"))
        .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "750xx0000000001AAA",
            "state": "UploadComplete",
            "operation": "upsert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "externalIdFieldName": "External_Id__c"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/jobs/ingest/750xx0000000001AAA"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "750xx0000000001AAA",
            "state": "JobComplete",
            "operation": "upsert",
            "object": "Account",
            "createdDate": "2024-01-01T00:00:00.000Z",
            "createdById": "005xx0000000001AAA",
            "externalIdFieldName": "External_Id__c",
            "numberRecordsProcessed": 2,
            "numberRecordsFailed": 0
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = applier
        .apply_bulk_upsert(
            "Account",
            "External_Id__c",
            1,
            vec![
                BulkAccountRow {
                    external_id: "external-1".to_owned(),
                    name: "Acme".to_owned(),
                },
                BulkAccountRow {
                    external_id: "external-2".to_owned(),
                    name: "Acme 2".to_owned(),
                },
            ],
        )
        .await
        .unwrap_or_else(|error| panic!("unexpected bulk apply error: {error}"));

    assert_eq!(result.jobs[0].id, "750xx0000000001AAA");
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn detect_drift_finds_hash_mismatches() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = PgStore::new(pool.clone());
    let envelope = envelope(
        "external-drift",
        json!({"Name": "Acme v2"}),
        SourceSystem::Postgres,
        ChangeOperation::Upsert,
        SourceCursor::PostgresLsn("lsn-1".to_owned()),
    );
    let journal_id = store.append_journal(&envelope).await?;

    let link = SyncLink {
        tenant: "tenant".to_owned(),
        object_name: "Account".to_owned(),
        external_id: "external-drift".to_owned(),
        salesforce_id: Some("001000000000001AAA".to_owned()),
        postgres_id: None,
        last_source: Some("postgres".to_owned()),
        last_source_cursor: Some("postgres-lsn:lsn-1".to_owned()),
        last_payload_hash: Some(vec![0, 1, 2, 3]),
        tombstone: false,
    };
    store.put_link(&link).await?;

    let drift: Vec<force_sync::DriftItem> = detect_drift(&store, 10).await?;
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].journal_id, journal_id);
    assert_eq!(drift[0].external_id, "external-drift");
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn reconcile_repair_enqueues_a_new_apply_task() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = PgStore::new(pool.clone());
    let envelope = envelope(
        "external-repair",
        json!({"Name": "Acme"}),
        SourceSystem::Postgres,
        ChangeOperation::Upsert,
        SourceCursor::PostgresLsn("lsn-2".to_owned()),
    );
    let journal_id = store.append_journal(&envelope).await?;

    let task_id = enqueue_repair(&store, journal_id).await?;
    let client = pool.get().await?;
    let row = client
        .query_one(
            "select task_kind, target_key, payload->>'journal_id'
             from sync_task
             where task_id = $1",
            &[&task_id],
        )
        .await?;

    assert_eq!(row.get::<_, String>(0), "apply");
    assert_eq!(row.get::<_, String>(1), journal_id.to_string());
    assert_eq!(row.get::<_, String>(2), journal_id.to_string());
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn run_reconcile_once_queues_repairs_for_drift() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = PgStore::new(pool.clone());
    let envelope = envelope(
        "external-reconcile",
        json!({"Name": "Acme v2"}),
        SourceSystem::Postgres,
        ChangeOperation::Upsert,
        SourceCursor::PostgresLsn("lsn-3".to_owned()),
    );
    let _ = store.append_journal(&envelope).await?;
    store
        .put_link(&SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "external-reconcile".to_owned(),
            salesforce_id: Some("001000000000001AAA".to_owned()),
            postgres_id: None,
            last_source: Some("postgres".to_owned()),
            last_source_cursor: Some("postgres-lsn:lsn-3".to_owned()),
            last_payload_hash: Some(vec![9, 9, 9]),
            tombstone: false,
        })
        .await?;

    assert_eq!(run_reconcile_once(&store, 10).await?, 1);

    let client = pool.get().await?;
    let task_count = client
        .query_one(
            "select count(*) from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task_count.get::<_, i64>(0), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires FORCE_SYNC_TEST_DATABASE_URL"]
async fn repeated_reconcile_passes_do_not_duplicate_repair_work() -> Result<(), ForceSyncError> {
    let pool = support::postgres::test_pool();
    support::postgres::reset_schema(&pool).await?;
    force_sync::migrate(&pool).await?;

    let store = PgStore::new(pool.clone());
    let envelope = envelope(
        "external-repeat",
        json!({"Name": "Acme v2"}),
        SourceSystem::Postgres,
        ChangeOperation::Upsert,
        SourceCursor::PostgresLsn("lsn-4".to_owned()),
    );
    let _ = store.append_journal(&envelope).await?;
    store
        .put_link(&SyncLink {
            tenant: "tenant".to_owned(),
            object_name: "Account".to_owned(),
            external_id: "external-repeat".to_owned(),
            salesforce_id: Some("001000000000001AAA".to_owned()),
            postgres_id: None,
            last_source: Some("postgres".to_owned()),
            last_source_cursor: Some("postgres-lsn:lsn-4".to_owned()),
            last_payload_hash: Some(vec![8, 8, 8]),
            tombstone: false,
        })
        .await?;

    assert_eq!(run_reconcile_once(&store, 10).await?, 1);
    assert_eq!(run_reconcile_once(&store, 10).await?, 1);

    let client = pool.get().await?;
    let task_count = client
        .query_one(
            "select count(*) from sync_task where task_kind = 'apply'",
            &[],
        )
        .await?;
    assert_eq!(task_count.get::<_, i64>(0), 1);
    Ok(())
}
