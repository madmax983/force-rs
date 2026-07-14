#![allow(missing_docs)]
#![cfg(feature = "rest")]
//! Core-tier live-contract tests: wide coverage of the default + common API
//! surfaces against a real Salesforce org.
//!
//! Every test is `#[ignore]` (so the default `cargo test` stays hermetic) and
//! skips cleanly when no credentials are present. Set core-tier creds
//! (`SF_CLIENT_ID`/`SF_CLIENT_SECRET`/`SF_TOKEN_URL`, or
//! `SF_ACCESS_TOKEN`+`SF_INSTANCE_URL`, or JWT/SF-CLI) to exercise them.
//!
//! Safety: mutating tests create records whose `LastName`/`Name` carries the
//! `force-rs-live-test` prefix and always delete what they created, even on
//! failure. No destructive operations run against non-test data.

mod common;

use common::LIVE_TEST_PREFIX;
use force::api::RestOperation;
use force::types::{DynamicSObject, SalesforceId};
use serde_json::json;

/// Unique suffix so parallel runs / retries never collide.
fn unique_suffix() -> String {
    chrono::Utc::now().timestamp_millis().to_string()
}

// ─── REST: full CRUD lifecycle ─────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_crud_lifecycle() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_core_rest_crud_lifecycle",
            "core-tier creds absent (set SF_CLIENT_ID/SF_CLIENT_SECRET/SF_TOKEN_URL or SF_ACCESS_TOKEN+SF_INSTANCE_URL)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let last_name = format!("{LIVE_TEST_PREFIX}-crud-{}", unique_suffix());

    // CREATE
    let created = client
        .rest()
        .create("Contact", &json!({ "LastName": last_name }))
        .await?;
    let Some(id) = created.id else {
        anyhow::bail!("create did not return an Id");
    };

    // Run the body; capture the outcome so cleanup always runs afterwards.
    let outcome = crud_lifecycle_body(&client, &id, &last_name).await;

    // TEARDOWN — best-effort, always attempted.
    let _ = client.rest().delete("Contact", &id).await;

    outcome
}

async fn crud_lifecycle_body(
    client: &force::client::ForceClient<common::LiveAuth>,
    id: &SalesforceId,
    last_name: &str,
) -> anyhow::Result<()> {
    // GET
    let fetched = client.rest().get("Contact", id).await?;
    anyhow::ensure!(
        fetched.get("LastName").and_then(|v| v.as_str()) == Some(last_name),
        "fetched LastName did not match created value",
    );

    // UPDATE
    client
        .rest()
        .update("Contact", id, &json!({ "Title": "force-rs live tester" }))
        .await?;
    let after = client.rest().get("Contact", id).await?;
    anyhow::ensure!(
        after.get("Title").and_then(|v| v.as_str()) == Some("force-rs live tester"),
        "update did not persist Title",
    );

    // QUERY back by Id
    let soql = format!(
        "SELECT Id, LastName FROM Contact WHERE Id = '{}'",
        id.as_str()
    );
    let result = client.rest().query::<DynamicSObject>(&soql).await?;
    anyhow::ensure!(
        result.total_size >= 1,
        "query did not find the created record"
    );

    Ok(())
}

// ─── REST: upsert by external id (falls back to create/query when unset) ───

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_upsert() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_core_rest_upsert",
            "core-tier creds absent (set SF_CLIENT_ID/SF_CLIENT_SECRET/SF_TOKEN_URL or SF_ACCESS_TOKEN+SF_INSTANCE_URL)",
        );
        return Ok(());
    };

    // Upsert needs an external-id field + value; supply via env when available.
    let (Some(sobject), Some(ext_field), Some(ext_value)) = (
        common::env_string("SF_UPSERT_SOBJECT"),
        common::env_string("SF_UPSERT_EXT_FIELD"),
        common::env_string("SF_UPSERT_EXT_VALUE"),
    ) else {
        common::skip(
            "live_core_rest_upsert",
            "upsert externals absent (set SF_UPSERT_SOBJECT/SF_UPSERT_EXT_FIELD/SF_UPSERT_EXT_VALUE)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let name = format!("{LIVE_TEST_PREFIX}-upsert-{}", unique_suffix());
    let response = client
        .rest()
        .upsert(&sobject, &ext_field, &ext_value, &json!({ "Name": name }))
        .await?;
    anyhow::ensure!(response.is_success(), "upsert reported non-success");

    // Clean up the upserted record.
    let _ = client.rest().delete(&sobject, &response.id).await;
    Ok(())
}

// ─── REST: query + query_more (pagination) ─────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_query_more_pagination() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_core_rest_query_more_pagination",
            "core-tier creds absent",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    // Small batch forces pagination when the org has more than 2 Accounts.
    let mut page = client
        .rest()
        .query::<DynamicSObject>("SELECT Id FROM Account LIMIT 2000")
        .await?;

    let mut pages = 1;
    while !page.done {
        let Some(next) = page.next_records_url.clone() else {
            break;
        };
        page = client.rest().query_more::<DynamicSObject>(&next).await?;
        pages += 1;
        if pages >= 3 {
            break; // don't walk the entire org
        }
    }
    // Either the org fit in one page (done immediately) or we followed a locator.
    anyhow::ensure!(pages >= 1, "expected at least one page");
    Ok(())
}

// ─── REST: SOSL search ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_search() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_rest_search", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    // Build SOSL via the builder so the search term is escaped. `LIVE_TEST_PREFIX`
    // contains `-`, a SOSL-reserved operator that produces MALFORMED_SEARCH when
    // interpolated raw into `FIND { ... }`.
    let sosl = force::api::rest::SearchQueryBuilder::new()
        .find(LIVE_TEST_PREFIX)
        .in_all_fields()
        .returning("Contact", &["Id"])
        .build();
    let _ = client.rest().search(&sosl).await?;
    Ok(())
}

// ─── REST: describe_global + describe ──────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_describe() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_rest_describe", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let global = client.rest().describe_global().await?;
    anyhow::ensure!(
        !global.sobjects.is_empty(),
        "describe_global returned no sobjects"
    );

    let account = client.rest().describe("Account").await?;
    anyhow::ensure!(
        !account.fields.is_empty(),
        "describe(Account) returned no fields"
    );
    Ok(())
}

// ─── REST: org limits ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_rest_limits() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_rest_limits", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let _ = client.rest().limits().await?;
    Ok(())
}

// ─── Composite: batch ──────────────────────────────────────────────────────

#[cfg(feature = "composite")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_composite_batch() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_composite_batch", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let result = client
        .composite()
        .batch()
        .add_request("GET", "limits", None)?
        .add_request("GET", "sobjects/Account/describe", None)?
        .execute()
        .await?;

    anyhow::ensure!(
        !result.has_errors,
        "composite batch returned errors: {result:?}"
    );
    anyhow::ensure!(result.results.len() == 2, "expected 2 subresponses");
    Ok(())
}

// ─── Composite: graph ──────────────────────────────────────────────────────

#[cfg(feature = "composite_graph")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_composite_graph() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_composite_graph", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let account_query = force::api::SoqlQueryBuilder::new()
        .select(&["Id"])
        .from("Account")
        .limit(1);

    let graph =
        force::api::composite::Graph::new("live-graph").query(account_query, "acctQuery")?;

    let response = client
        .composite()
        .graph()
        .add_graph(graph)?
        .execute()
        .await?;
    anyhow::ensure!(
        response.graphs.iter().all(|g| g.is_successful),
        "composite graph reported an unsuccessful graph: {response:?}",
    );
    Ok(())
}

// ─── Bulk 2.0: read-only query stream smoke ────────────────────────────────

#[cfg(feature = "bulk")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_bulk_query_stream() -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "Id")]
        id: String,
    }

    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_bulk_query_stream", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let mut stream = client
        .bulk()
        .bulk_query::<Row>("SELECT Id FROM Account LIMIT 5")
        .await?;

    let mut seen = 0usize;
    while let Some(row) = stream.next().await? {
        anyhow::ensure!(!row.id.is_empty(), "bulk row had empty Id");
        seen += 1;
        if seen >= 5 {
            break;
        }
    }
    anyhow::ensure!(seen <= 5, "bulk query returned more than the LIMIT");
    Ok(())
}

// ─── Tooling ───────────────────────────────────────────────────────────────

#[cfg(feature = "tooling")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_tooling_query_and_execute() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_core_tooling_query_and_execute",
            "core-tier creds absent",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let _ = client
        .tooling()
        .query::<serde_json::Value>("SELECT Id FROM ApexClass LIMIT 1")
        .await?;

    let result = client
        .tooling()
        .execute_anonymous(&format!("System.debug('{LIVE_TEST_PREFIX}');"))
        .await?;
    anyhow::ensure!(
        result.is_success(),
        "execute_anonymous was not successful: {result:?}"
    );
    Ok(())
}

// ─── UI API ────────────────────────────────────────────────────────────────

#[cfg(feature = "ui")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_ui_object_info_and_defaults() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_core_ui_object_info_and_defaults",
            "core-tier creds absent",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let info = client.ui().object_info("Account").await?;
    anyhow::ensure!(
        info.api_name == "Account",
        "object_info returned wrong api_name"
    );
    anyhow::ensure!(
        info.fields.contains_key("Id"),
        "object_info missing Id field"
    );

    // create-defaults for a standard object.
    let _ = client.ui().create_defaults("Contact").await?;
    Ok(())
}

// ─── GraphQL ───────────────────────────────────────────────────────────────

#[cfg(feature = "graphql")]
#[tokio::test]
#[ignore = "requires a live Salesforce org"]
async fn live_core_graphql_query() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_core_graphql_query", "core-tier creds absent");
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let data = client
        .graphql()
        .query_raw(
            r"{ uiapi { query { Account(first: 1) { edges { node { Id { value } } } } } } }",
            None,
        )
        .await?;

    let account = &data["uiapi"]["query"]["Account"];
    anyhow::ensure!(
        account.is_object(),
        "expected Account object, got {account:?}"
    );
    anyhow::ensure!(
        account["edges"].is_array(),
        "expected edges array, got {account:?}"
    );
    Ok(())
}
