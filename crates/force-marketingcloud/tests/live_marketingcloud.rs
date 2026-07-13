#![allow(missing_docs)]
//! Marketing Cloud Engagement live-contract smoke.
//!
//! Read-only: gated on the `MC_*` env vars. Builds an Installed-Package
//! server-to-server client and performs the simplest read call (list Content
//! Builder assets). `#[ignore]` keeps the default `cargo test` hermetic; the
//! test skips cleanly (printing the required vars) when credentials are absent.
//!
//! Required: `MC_TENANT_SUBDOMAIN`, `MC_CLIENT_ID`, `MC_CLIENT_SECRET`.
//! Optional: `MC_ACCOUNT_ID` (MID), `MC_SCOPE`, `MC_AUTH_URL`.

use force_marketingcloud::MarketingCloudClient;

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    })
}

#[tokio::test]
#[ignore = "requires live Marketing Cloud Engagement credentials"]
async fn live_mc_assets_list() -> anyhow::Result<()> {
    let (Some(subdomain), Some(client_id), Some(client_secret)) = (
        env_string("MC_TENANT_SUBDOMAIN"),
        env_string("MC_CLIENT_ID"),
        env_string("MC_CLIENT_SECRET"),
    ) else {
        eprintln!(
            "SKIP live_mc_assets_list: Marketing Cloud creds absent (set MC_TENANT_SUBDOMAIN, MC_CLIENT_ID, MC_CLIENT_SECRET)"
        );
        return Ok(());
    };

    let mut builder = MarketingCloudClient::builder()
        .tenant_subdomain(subdomain)
        .client_credentials(client_id, client_secret);

    if let Some(account_id) = env_string("MC_ACCOUNT_ID") {
        builder = builder.account_id(account_id);
    }
    if let Some(scope) = env_string("MC_SCOPE") {
        builder = builder.scope(scope);
    }
    if let Some(auth_url) = env_string("MC_AUTH_URL") {
        builder = builder.auth_url(auth_url);
    }

    let client = builder.build()?;
    let _ = client.assets().list().await?;
    Ok(())
}
