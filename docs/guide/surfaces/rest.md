# REST API

The core Salesforce Platform REST API over `/services/data/vXX.0/`: SOQL queries,
CRUD, upsert, SOSL search, Describe metadata, org limits, and query-plan analysis.
This is the default surface — everything else builds on the same session and HTTP
stack.

- **Feature flag:** `rest` (default, on unless you opt out)
- **Accessor:** `client.rest()` → `RestHandler`

CRUD/Query/Describe live on the shared [`RestOperation`] trait, so it must be in
scope:

```rust
use force::api::rest_operation::RestOperation; // or the re-export: force::api::RestOperation
```

## Query

`query::<T>` deserializes records into any `T: DeserializeOwned` (use
`serde_json::Value` or `force::types::DynamicSObject` for dynamic results).
Paginate with `query_more::<T>` using the `next_records_url` from the prior page.

```rust
let result = client.rest()
    .query::<Account>("SELECT Id, Name, Industry FROM Account LIMIT 10")
    .await?;

let mut page = result;
while let Some(next) = page.next_records_url.clone() {
    page = client.rest().query_more::<Account>(&next).await?;
}
```

For hands-off pagination, `query_stream::<T>(soql)` returns a `Stream` that fetches
pages lazily.

> SOQL strings are raw. Never `format!` untrusted input into a query — use bind
> values / `SoqlQueryBuilder` and `escape_soql` instead.

## CRUD & upsert

```rust
let created = client.rest().create("Account", &json!({ "Name": "Acme" })).await?;
let id = created.id.unwrap();

let account = client.rest().get("Account", &id).await?;
client.rest().update("Account", &id, &json!({ "Industry": "Software" })).await?;
client.rest().delete("Account", &id).await?;

// Upsert by external ID. `upsert_idempotent` opts into retry-on-503.
client.rest()
    .upsert("Account", "ExternalId__c", "ACME-001", &json!({ "Name": "Acme" }))
    .await?;
```

Note: `get` takes a `SalesforceId` (the `CreateResponse.id` field). An upsert that
updates an existing record returns 204 with no body, surfaced as
`ForceError::NotImplemented` — re-query for the ID.

## Search, Describe, limits, explain

```rust
let hits    = client.rest().search("FIND {Acme} IN ALL FIELDS RETURNING Account(Id, Name)").await?;
let global  = client.rest().describe_global().await?;   // all SObjects
let account = client.rest().describe("Account").await?; // one SObject's fields
let limits  = client.rest().limits().await?;             // org limits
let plan    = client.rest().explain("SELECT Id FROM Account WHERE Name = 'x'").await?;
```

## See also

- Auth setup: [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- Retries, errors, pagination: [Operations](../03-operations.md)
- [ADR-007 — REST API design](../../adr/007-rest-api-design.md)
- [ADR-019 — RestOperation trait](../../adr/019-tooling-api-design.md)
- Examples:
  [`basic_crud.rs`](../../../crates/force/examples/basic_crud.rs),
  [`soql_query.rs`](../../../crates/force/examples/soql_query.rs),
  [`dynamic_query.rs`](../../../crates/force/examples/dynamic_query.rs),
  [`search.rs`](../../../crates/force/examples/search.rs),
  [`describe.rs`](../../../crates/force/examples/describe.rs),
  [`org_limits.rs`](../../../crates/force/examples/org_limits.rs),
  [`query_plan.rs`](../../../crates/force/examples/query_plan.rs),
  [`readme_quick_start.rs`](../../../crates/force/examples/readme_quick_start.rs)
- Rustdoc: `cargo doc --no-deps --open` → `force::api::rest`

[`RestOperation`]: ../../../crates/force/src/api/rest_operation.rs
