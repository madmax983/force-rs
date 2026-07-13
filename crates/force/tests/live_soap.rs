#![allow(missing_docs)]
#![cfg(feature = "soap")]
//! SOAP Partner API live-contract tests against a real Salesforce org.
//!
//! These share the core-tier credential loader in `tests/common/mod.rs` (SOAP
//! reuses the OAuth token in its `SessionHeader`, so no extra secrets are
//! needed). Every test is `#[ignore]` so the default `cargo test` stays
//! hermetic, and each skips cleanly when no credentials are present.
//!
//! Safety: the mutating round-trip creates a `Contact` whose `LastName` carries
//! the `force-rs-live-test` prefix and always deletes it, even on failure. The
//! remaining tests are read-only smokes.

mod common;

use common::LIVE_TEST_PREFIX;

/// Unique suffix so parallel runs / retries never collide.
fn unique_suffix() -> String {
    chrono::Utc::now().timestamp_millis().to_string()
}

// ─── SOAP: getServerTimestamp (zero side effects) ──────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_server_timestamp() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_server_timestamp", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    // Parsing a valid RFC 3339 timestamp is enforced inside get_server_timestamp.
    let _ = client.soap().get_server_timestamp().await?;
    Ok(())
}

// ─── SOAP: getUserInfo ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_get_user_info() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_get_user_info", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let info = client.soap().get_user_info().await?;
    anyhow::ensure!(
        !info.user_id.is_empty(),
        "getUserInfo returned empty user id"
    );
    anyhow::ensure!(
        !info.organization_id.is_empty(),
        "getUserInfo returned empty organization id"
    );
    Ok(())
}

// ─── SOAP: describeGlobal ──────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_describe_global() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_describe_global", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let global = client.soap().describe_global().await?;
    anyhow::ensure!(
        !global.sobjects.is_empty(),
        "describe_global returned no sobjects"
    );
    Ok(())
}

// ─── SOAP: describeSObject ─────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_describe_sobject() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_describe_sobject", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let account = client.soap().describe_sobject("Account").await?;
    anyhow::ensure!(
        !account.fields.is_empty(),
        "describe_sobject(Account) returned no fields"
    );
    Ok(())
}

// ─── SOAP: query (SOQL) ────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_query() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_query", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    // Parsing the response (done flag + records) is exercised by the call itself.
    let result = client
        .soap()
        .query("SELECT Id FROM Account LIMIT 1")
        .await?;
    anyhow::ensure!(
        result.records.len() <= 1,
        "query returned more than the LIMIT"
    );
    Ok(())
}

// ─── SOAP: CRUD round-trip (create → body → best-effort delete) ────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_soap_crud_roundtrip() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_soap_crud_roundtrip", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let last_name = format!("{LIVE_TEST_PREFIX}-soap-{}", unique_suffix());

    // CREATE
    let record = force::api::soap::SObject::new("Contact").with_field("LastName", &last_name);
    let saves = client.soap().create(&[record]).await?;
    anyhow::ensure!(saves.len() == 1, "expected exactly one SaveResult");
    anyhow::ensure!(
        saves[0].success,
        "create reported failure: {:?}",
        saves[0].errors
    );
    let Some(id) = saves[0].id.clone() else {
        anyhow::bail!("create did not return an Id");
    };

    // Run the body; capture the outcome so cleanup always runs afterwards.
    let outcome = soap_crud_body(&client, &id, &last_name).await;

    // TEARDOWN — best-effort, always attempted.
    let _ = client.soap().delete(&[id]).await;

    outcome
}

async fn soap_crud_body(
    client: &force::client::ForceClient<common::LiveAuth>,
    id: &str,
    last_name: &str,
) -> anyhow::Result<()> {
    // Typed row for the typed-layer smoke below. Hoisted to the top of the
    // scope so it precedes all statements (clippy::items_after_statements).
    #[derive(serde::Deserialize)]
    struct Contact {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "LastName")]
        last_name: String,
    }

    // RETRIEVE by Id and confirm the round-tripped LastName.
    let retrieved = client
        .soap()
        .retrieve("Contact", &["Id", "LastName"], &[id])
        .await?;
    anyhow::ensure!(
        retrieved
            .iter()
            .any(|r| r.get("LastName") == Some(last_name)),
        "retrieve did not return the created LastName"
    );

    // QUERY back by Id.
    let soql = format!("SELECT Id, LastName FROM Contact WHERE Id = '{id}'");
    let queried = client.soap().query(&soql).await?;
    anyhow::ensure!(!queried.records.is_empty(), "query did not find the record");

    // Smoke the typed layer with the same query.
    let typed: Vec<Contact> = client.soap().query_typed(&soql).await?;
    anyhow::ensure!(
        typed.iter().any(|c| c.id == id && c.last_name == last_name),
        "typed query did not round-trip the created Contact"
    );

    Ok(())
}
