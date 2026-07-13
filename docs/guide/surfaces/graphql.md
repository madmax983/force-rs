# GraphQL API

Unified query interface over `/services/data/vXX.0/graphql`: request specific
fields, traverse relationships, and query multiple objects in a single POST.
Handler wraps a custom deserialization pipeline that surfaces GraphQL errors as
`Result` failures (or lets you inspect partial success).

- **Feature flag:** `graphql`
- **Accessor:** `client.graphql()` → `GraphqlHandler`

```toml
force = { version = "...", features = ["graphql"] }
```

## Query

Build requests with `GraphqlRequest::new(...)`, optionally chaining
`.with_variables(json)` and `.with_operation_name(...)`.

```rust
use force::api::graphql::GraphqlRequest;

let request = GraphqlRequest::new(r#"
    query { uiapi { query { Account(first: 5) {
        edges { node { Id Name { value } } }
    } } } }
"#);

// Typed: errors-without-data become Err(ForceError::GraphQL)
let data: MyType = client.graphql().query(&request).await?;
```

## Variants

```rust
// Full envelope (data + errors) for partial success / extensions
let envelope = client.graphql().query_with_errors::<MyType>(&request).await?;

// Raw string → serde_json::Value (data field only)
let value = client.graphql().query_raw("query { ... }", None).await?;
```

## See also

- [ADR-021 — GraphQL API design](../../adr/021-graphql-api-design.md)
- Example: [`crates/force/examples/graphql.rs`](../../../crates/force/examples/graphql.rs)
- Rustdoc: `cargo doc --no-deps --features graphql --open` → `force::api::graphql`
