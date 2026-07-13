# Apex REST API

Generic access to custom Apex REST endpoints under
`/services/apexrest/{path}`. Version-less URL construction; you supply the path
after `/apexrest/` and a body/response type. Use this for any hand-written
`@RestResource` class.

- **Feature flag:** `apex_rest`
- **Accessor:** `client.apex_rest()` → `ApexRestHandler`

```toml
force = { version = "...", features = ["apex_rest"] }
```

## Methods

`get`, `get_with_params`, `post`, `post_raw`, `patch`, `put`, `delete`. All
generic over `DeserializeOwned` responses (`delete` returns `()`).

```rust
// POST /services/apexrest/MyNamespace/MyEndpoint
let result: MyResponse = client.apex_rest()
    .post("MyNamespace/MyEndpoint", &request_body)
    .await?;

// GET with query params
let records: Vec<MyRecord> = client.apex_rest()
    .get_with_params("MyService/records", &[("limit", "10")])
    .await?;

// Dynamic response shape
let value = client.apex_rest().post_raw("MyService/action", &body).await?;
```

## See also

- [ADR-023 — Apex REST + CPQ design](../../adr/023-apex-rest-cpq-design.md)
- Rustdoc: `cargo doc --no-deps --features apex_rest --open` → `force::api::apex_rest`
