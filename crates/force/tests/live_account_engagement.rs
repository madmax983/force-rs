#![allow(missing_docs)]
#![cfg(feature = "account_engagement")]
//! Account Engagement (Pardot) API v5 live-contract smoke.
//!
//! Read-mostly: gated on core-tier creds plus `SF_AE_BUSINESS_UNIT_ID`. The
//! handler targets the Pardot host (`pi.pardot.com` / `pi.demo.pardot.com`)
//! derived from the client environment and sends the required
//! `Pardot-Business-Unit-Id` header. `#[ignore]` keeps the default run hermetic.

mod common;

#[tokio::test]
#[ignore = "requires a live Account Engagement (Pardot) business unit"]
async fn live_ae_query_lists() -> anyhow::Result<()> {
    let Some(cfg) = common::load_core_config() else {
        common::skip("live_ae_query_lists", "core-tier creds absent");
        return Ok(());
    };
    let Some(bu_id) = common::account_engagement_bu() else {
        common::skip(
            "live_ae_query_lists",
            "business unit absent (set SF_AE_BUSINESS_UNIT_ID)",
        );
        return Ok(());
    };

    let client = common::create_client(&cfg).await?;
    // `fields` is mandatory for AE v5; keep the projection minimal.
    let _ = client
        .account_engagement(bu_id)
        .query_lists("id,name", &[("limit", "5")])
        .await?;
    Ok(())
}
