# force-marketingcloud

`force-marketingcloud` is a standalone, REST-first client for the Salesforce
**Marketing Cloud Engagement** (formerly ExactTarget) platform.

It is intentionally decoupled from the core `force` crate: Marketing Cloud uses a
wholly separate authentication model (a per-tenant auth subdomain, Installed
Package JSON client credentials, ~20-minute tokens with **no refresh token**, and
business-unit / MID tenancy), so it does not share the core client's session or
auth types. See [ADR-034](../../docs/adr/034-marketing-cloud-engagement-crate.md).

## Scope

- **Auth** — Installed-Package server-to-server (client credentials) flow with a
  proactive, per-business-unit token cache and single-flight refresh.
- **Transactional Messaging** — single-recipient email and SMS sends, plus status.
- **Content Builder Assets** — create / get / update / delete / list.
- **Contacts** — create, delete by contact key.
- **Data Extensions** — sync rowset upsert, async row insert, and row queries.
- **Journeys (Interaction)** — list journeys and fire entry events.
- **Raw escape hatch** — `raw_get` / `raw_post` / `raw_request` for any other
  endpoint relative to the token's `rest_instance_url`.

SOAP is out of scope; the SOAP endpoint URL is surfaced on the token but a typed
SOAP client is a possible follow-up.

## Quick start

```rust,no_run
use force_marketingcloud::{MarketingCloudClient, Recipient, SendEmailRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = MarketingCloudClient::builder()
        .tenant_subdomain("mc9xxxxxxxxxxxxxxxx")
        .client_credentials("client-id", "client-secret")
        .account_id("1234567") // optional default business unit (MID)
        .build()?;

    let request = SendEmailRequest::new(
        "transactional_welcome",
        Recipient::new("contact-001", "jane@example.com"),
    );
    let response = client
        .transactional()
        .send_email("MSG-KEY-123", &request)
        .await?;
    println!("request id: {:?}", response.request_id);

    // Target a different business unit for a single call:
    let journeys = client.journeys().for_business_unit("7654321").list().await?;
    println!("{} journeys", journeys.items.len());

    Ok(())
}
```

## License

MIT OR Apache-2.0
