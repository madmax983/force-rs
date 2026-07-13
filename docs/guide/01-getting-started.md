# Getting Started

## Install

Add `force` to your `Cargo.toml`. The default feature is `rest`; enable additional
API surfaces and auth flows explicitly.

```toml
[dependencies]
force = { version = "0.3", features = ["rest", "bulk", "jwt"] }
```

To opt out of the default REST surface, disable default features and select only
what you need:

```toml
[dependencies]
force = { version = "0.3", default-features = false, features = ["tooling"] }
```

## Feature-flag matrix

Every gate defined in `crates/force/Cargo.toml`. The "Pulls in" column lists the
literal dependency edges from the `[features]` table.

| Feature | Pulls in | Surface / capability |
| --- | --- | --- |
| `default` | `rest` | Default feature set (REST only). |
| `rest` | — | REST API: SOQL/SOSL, CRUD, describe, query plan. |
| `files` | `rest` | Salesforce Files (ContentVersion/Document) helpers. |
| `tooling` | — | Tooling API: Apex classes, execute anonymous, run tests, completions. |
| `bulk` | `dep:csv` | Bulk API 2.0 (CSV ingest/query jobs). |
| `composite` | `rest` | Composite API (batch, tree, sub-requests). |
| `composite_graph` | `composite` | Composite Graph API (dependency-ordered graphs). |
| `schema` | `rest` | Schema tooling: Iceberg schema generation, struct/data-dictionary generators. |
| `data_utility` | `composite` | Data utilities layered on Composite (mass SOQL ops). |
| `jwt` | `dep:jsonwebtoken` | JWT Bearer auth flow (server-to-server). |
| `username_password` | — | Username-password auth flow (deprecated by Salesforce; gated as a speed bump). |
| `auth_code` | `dep:sha2`, `dep:getrandom` | OAuth 2.0 Authorization Code + PKCE flow. |
| `mock` | `dep:wiremock` | Testing utilities (wiremock-backed doubles). |
| `ui` | — | UI API: layout-aware records, object info, list views, lookups, favorites. |
| `graphql` | — | GraphQL API: typed queries, variables, partial-success envelopes. |
| `data_cloud` | — | Data Cloud REST Connect API (SQL query, two-step token exchange). |
| `apex_rest` | — | Generic Apex REST access (`/services/apexrest/`). |
| `cpq` | `apex_rest` | Salesforce CPQ API (quote lifecycle, config, documents, amendments). |
| `consent` | — | Consent & Portability API (GDPR/CCPA consent checks, data export). |
| `models` | — | Agentforce Models API (Einstein LLM gateway on `api.salesforce.com`). |
| `agent_api` | `dep:uuid` | Agentforce Agent API (headless agent sessions on `api.salesforce.com`). |
| `agentforce` | `models`, `agent_api` | Umbrella for the full Agentforce surface. |
| `account_engagement` | — | Account Engagement (Pardot) API v5 on `pi.pardot.com`. |
| `analytics` | — | Reports & Dashboards REST API. |
| `full` | see below | Common set of surfaces + auth flows. |
| `all` | `full` + extras | Everything, including specialized/heavier surfaces. |

### `full` vs `all`

From the actual definitions:

```toml
full = ["rest", "files", "bulk", "composite", "tooling", "jwt", "auth_code",
        "ui", "graphql", "data_cloud", "apex_rest", "consent", "models",
        "agent_api", "account_engagement", "analytics"]
all  = ["full", "schema", "data_utility", "composite_graph", "cpq"]
```

- `full` is the common set: the standard API surfaces plus the `jwt` and
  `auth_code` flows. It deliberately omits `schema`, `data_utility`,
  `composite_graph`, and `cpq`.
- `all` is `full` plus `schema`, `data_utility`, `composite_graph`, and `cpq`.

Note `username_password` and `mock` are in neither meta-feature; enable them
explicitly.

### Pub/Sub is not a `force` feature

There is no `pub_sub` gate in `force`. The gRPC Pub/Sub API lives in the separate
`force-pubsub` crate. See [sibling crates](surfaces/sibling-crates.md).

## MSRV

Rust **1.92** (edition **2024**), set by `rust-version` in the workspace
`Cargo.toml`. The `force-lake` crate raised the workspace MSRV to 1.92 (it depends
on `iceberg` 0.9 / S3 Tables). Building only `force` still requires 1.92 because the
workspace pins a single `rust-version`.

## TLS / rustls

`force` uses **rustls**, not OpenSSL. `reqwest` is configured workspace-wide with
`default-features = false, features = ["rustls-tls", "json"]`, so there is no
OpenSSL/native-tls dependency.

### Crypto-provider interaction with `force-lake`

rustls selects a process-wide default `CryptoProvider` (either `aws-lc-rs` or
`ring`). A binary can end up linking **two** provider backends:

- `force` links rustls via `reqwest`'s `rustls-tls` feature.
- `force-lake` pulls the AWS S3 Tables / Iceberg tree
  (`iceberg-catalog-s3tables` → `reqsign`), which brings `ring` transitively
  (documented in the workspace `deny.toml`).

When more than one crypto provider is compiled into the same binary, rustls cannot
pick a process-wide default on its own and will **panic at runtime** on the first
TLS handshake ("no process-level CryptoProvider available" / "multiple crypto
providers"). If you link both `force` and `force-lake` (or otherwise pull two
providers), install a default provider once at process startup before making any
requests, e.g.:

```rust
// Choose the provider your dependency tree actually compiles.
rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install rustls crypto provider");
```

This detail depends on your resolved dependency graph. If you use only `force`
(single provider), no manual installation is required. Inspect your `Cargo.lock`
to confirm which providers are present before choosing one to install.

## Quick start

Client-credentials auth plus a typed SOQL query (mirrors
`crates/force/examples/readme_quick_start.rs`):

```rust
use force::api::RestOperation;
use force::auth::ClientCredentials;
use force::client::ForceClientBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Authenticate with OAuth 2.0 client credentials.
    let auth = ClientCredentials::new_my_domain(
        "your-client-id",
        "your-client-secret",
        "https://your-org.my.salesforce.com",
    );

    let client = ForceClientBuilder::new().authenticate(auth).build().await?;

    // Execute a typed SOQL query.
    let soql = "SELECT Id, Name, Industry FROM Account WHERE Industry = 'Technology' LIMIT 10";
    let result = client.rest().query::<Account>(soql).await?;

    for account in result.records {
        println!(
            "{}: {} ({})",
            account.id,
            account.name,
            account.industry.unwrap_or_default()
        );
    }

    Ok(())
}
```

Next:

- [Choosing an auth flow](02-choosing-an-auth-flow.md)
- [REST surface](surfaces/rest.md)
