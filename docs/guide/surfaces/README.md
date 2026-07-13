# API Surfaces

`force-rs` exposes each Salesforce API surface as a feature-gated handler off the
`ForceClient`. Only the surfaces whose features you enable are compiled. Every
handler shares the same authenticated session, HTTP stack, retries, and error
types — see [Choosing an auth flow](../02-choosing-an-auth-flow.md) and
[Operations](../03-operations.md).

CRUD / Query / Describe on the REST and Tooling handlers come from the shared
`RestOperation` trait, which must be in scope:

```rust
use force::api::rest_operation::RestOperation; // or the re-export force::api::RestOperation
```

## Surfaces

| Surface | Accessor | Feature flag | Description |
|---------|----------|--------------|-------------|
| [REST](rest.md) | `client.rest()` | `rest` (default) | SOQL, CRUD, upsert, SOSL search, Describe, org limits |
| [Tooling](tooling.md) | `client.tooling()` | `tooling` | Apex/metadata CRUD + execute-anonymous, run-tests, completions |
| [Bulk](bulk.md) | `client.bulk()` | `bulk` | Bulk API 2.0 high-volume ingest & streaming query |
| [Composite](composite.md) | `client.composite()` | `composite` | Batch (25) & graph (500) requests in one round-trip |
| [UI](ui.md) | `client.ui()` | `ui` | Layout-aware records, object info, list views, lookups, favorites |
| [GraphQL](graphql.md) | `client.graphql()` | `graphql` | Field-precise queries & relationships via a single POST |
| [Data Cloud](data-cloud.md) | `client.data_cloud()` | `data_cloud` | Data Cloud REST Connect SQL queries (two-step token exchange) |
| [Apex REST](apex-rest.md) | `client.apex_rest()` | `apex_rest` | Generic access to custom `/services/apexrest/` endpoints |
| [CPQ](cpq.md) | `client.cpq()` | `cpq` | Salesforce CPQ quote lifecycle, config, documents, amendments |
| [Consent](consent.md) | `client.consent()` | `consent` | GDPR/CCPA consent checks & data portability |
| [Analytics](analytics.md) | `client.analytics()` | `analytics` | Reports & Dashboards REST API (runs, instances, results) |
| [Account Engagement](account-engagement.md) | `client.account_engagement()` | `account_engagement` | Pardot API v5 (separate host, business-unit-scoped) |
| [SOAP](soap.md) | — | `soap` | Legacy SOAP API (planned — Phase 5 roadmap) |
| [Agentforce](agentforce.md) | `client.models()` / `client.agents()` | `agentforce` (= `models` + `agent_api`) | Einstein LLM gateway & headless agent sessions (`api.salesforce.com`) |
| [Sibling crates](sibling-crates.md) | separate crates | — | `force-pubsub`, `force-marketingcloud`, `force-sync`, `force-lake` |

Umbrella features: `full` enables the common surfaces; `all` adds specialized ones
(e.g. `cpq`). See the crate `Cargo.toml` and
[ADR-004 — feature gates](../../adr/004-feature-gates.md) for the exact sets.

## See also

- [Getting started](../01-getting-started.md)
- [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- [Operations — retries, errors, pagination](../03-operations.md)
