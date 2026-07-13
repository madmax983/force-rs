# Data Cloud API

SQL queries against Data Cloud (`ssot/`) via the REST Connect API. Data Cloud
uses a **two-step token exchange**: a `DataCloudAuthenticator` decorator swaps
the core org token for a Data Cloud instance token. Enable it on the builder
with `.with_data_cloud(...)`.

- **Feature flag:** `data_cloud`
- **Accessor:** `client.data_cloud()` → `Result<DataCloudHandler>`
  (errors if `.with_data_cloud()` was not called)

```toml
force = { version = "...", features = ["data_cloud"] }
```

## Setup + query

```rust
use force::auth::DataCloudConfig;

let client = ForceClientBuilder::new()
    .authenticate(auth)
    .with_data_cloud(DataCloudConfig::default())
    .build()
    .await?;

let dc = client.data_cloud()?;
let result = dc.query_sql(
    "SELECT Id, Name__c FROM UnifiedProfile__dlm LIMIT 10"
).await?;

for record in &result.data {
    println!("{record:?}");
}
```

Rows come back as `Vec<DataCloudRecord>` (`HashMap<String, Value>`); column
types are in `result.metadata`.

## See also

- [ADR-022 — Data Cloud API design](../../adr/022-data-cloud-api-design.md)
- Token exchange details: [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- Rustdoc: `cargo doc --no-deps --features data_cloud --open` → `force::api::data_cloud`
