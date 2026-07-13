#![allow(missing_docs)]
//! Special-surface live-contract tests: minimal, own-env-gated smokes for API
//! surfaces that need extra configuration (a provisioned tenant, a deployed
//! Apex endpoint, a configured agent, etc.).
//!
//! Each test is gated on BOTH core-tier creds AND its own tier env var, and
//! prints a specific SKIP reason naming the missing variable. Tests are
//! `#[ignore]`, so the default `cargo test` stays hermetic. Any mutation is
//! wrapped in created-record / created-session cleanup.

mod common;

// ─── Data Cloud ────────────────────────────────────────────────────────────

#[cfg(feature = "data_cloud")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with Data Cloud"]
async fn live_special_data_cloud_query() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_special_data_cloud_query", "core-tier creds absent");
        return Ok(());
    };
    let Some(dc_config) = common::load_data_cloud() else {
        common::skip(
            "live_special_data_cloud_query",
            "Data Cloud tier disabled (set SF_DATA_CLOUD=1)",
        );
        return Ok(());
    };

    let client_config = force::config::ClientConfig {
        api_version: cfg.api_version.clone(),
        ..Default::default()
    };
    let client = force::client::builder()
        .config(client_config)
        .authenticate(cfg.auth.clone())
        .with_data_cloud(dc_config)
        .build()
        .await?;

    let dc = client.data_cloud()?;
    let _ = dc.query_sql("SELECT 1").await?;
    Ok(())
}

// ─── Apex REST ─────────────────────────────────────────────────────────────

#[cfg(feature = "apex_rest")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with a deployed Apex REST endpoint"]
async fn live_special_apex_rest_get() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_special_apex_rest_get", "core-tier creds absent");
        return Ok(());
    };
    let Some(path) = common::apex_rest_path() else {
        common::skip(
            "live_special_apex_rest_get",
            "Apex REST path absent (set SF_APEX_REST_PATH)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let _ = client.apex_rest().get::<serde_json::Value>(&path).await?;
    Ok(())
}

// ─── Consent ───────────────────────────────────────────────────────────────

#[cfg(feature = "consent")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with Consent data"]
async fn live_special_consent_read() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_special_consent_read", "core-tier creds absent");
        return Ok(());
    };
    let Some((action, ids)) = common::consent_params() else {
        common::skip(
            "live_special_consent_read",
            "consent params absent (set SF_CONSENT_ACTION + SF_CONSENT_IDS)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let _ = client.consent().read_consent(&action, &id_refs).await?;
    Ok(())
}

// ─── Agentforce Models ─────────────────────────────────────────────────────

#[cfg(feature = "models")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with Agentforce Models enabled"]
async fn live_special_models_generate_text() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip(
            "live_special_models_generate_text",
            "core-tier creds absent",
        );
        return Ok(());
    };
    let Some(model) = common::models_model() else {
        common::skip(
            "live_special_models_generate_text",
            "model absent (set SF_MODELS_MODEL)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let request = force::api::models::GenerateTextRequest::new(format!(
        "Reply with the single word: {}",
        common::LIVE_TEST_PREFIX
    ));
    let _ = client.models().generate_text(&model, &request).await?;
    Ok(())
}

// ─── Agentforce Agent API ──────────────────────────────────────────────────

#[cfg(feature = "agent_api")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with an Agentforce agent"]
async fn live_special_agent_session() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_special_agent_session", "core-tier creds absent");
        return Ok(());
    };
    let Some(agent) = common::agent_id() else {
        common::skip(
            "live_special_agent_session",
            "agent id absent (set SF_AGENT_ID)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let session = client.agents().start_session_default(&agent).await?;

    // Best-effort teardown of the session we just opened.
    let _ = client
        .agents()
        .end_session(
            &session.session_id,
            force::api::agent_api::SessionEndReason::UserRequest,
        )
        .await;
    Ok(())
}

// ─── CPQ ───────────────────────────────────────────────────────────────────

#[cfg(feature = "cpq")]
#[tokio::test]
#[ignore = "requires a live Salesforce org with CPQ installed"]
async fn live_special_cpq_read_quote() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_special_cpq_read_quote", "core-tier creds absent");
        return Ok(());
    };
    let Some(quote_id) = common::cpq_quote_id() else {
        common::skip(
            "live_special_cpq_read_quote",
            "quote id absent (set SF_CPQ_QUOTE_ID)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    let _ = client.cpq().read_quote(&quote_id).await?;
    Ok(())
}
