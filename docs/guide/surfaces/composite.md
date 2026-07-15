# Composite API

Bundle multiple REST operations into a single round-trip over
`/services/data/vXX.0/composite/`. `batch` runs up to 25 independent subrequests;
the optional graph API runs up to 500 dependent nodes with rollback.

- **Feature flag:** `composite` (graph is behind `composite_graph`)
- **Accessor:** `client.composite()` → `CompositeHandler`

```toml
force = { version = "...", features = ["composite"] }
```

## Batch

`batch()` returns a builder. Each `get` / `post` / `patch` / `delete` returns
`Result<Self>` (they validate inputs), so chain with `?`. `execute()` sends the
bundle; inspect `has_errors` and per-subrequest `results`.

```rust
let response = client.composite()
    .batch()
    .get("Account", "001000000000001AAA")?
    .post("Contact", json!({ "LastName": "Doe" }))?
    .execute()
    .await?;

if response.has_errors { /* inspect */ }
for sub in &response.results {
    println!("{}: {:?}", sub.status_code, sub.result);
}
```

Builder also offers `.halt_on_error(bool)`, `.query(SoqlQueryBuilder)`,
`.add_request(...)`, and `.len()` / `.is_empty()` / `.is_full()` (25-subrequest cap).

## Graph & mass ops

> **Graph nodes must be sObject record operations.** The Composite Graph API
> only supports `sobjects/{type}[/{id}]` nodes (via `Graph::get`/`post`/`patch`/`delete`);
> the SOQL `/query` resource is **not** a valid graph node and the org rejects it
> with `OPERATION_NOT_ALLOWED`. `Graph::query` is retained for API completeness only.

```rust
// Dependent request graph (feature: composite_graph)
let graph = client.composite().graph();

// SoqlMassOp: query records, then update them all via chunked composite batches
use force::api::composite::SoqlMassOp;
let stats = SoqlMassOp::new(&client, "SELECT Id FROM Account WHERE Industry = 'Tech'")
    .halt_on_error(false)
    .update_all(json!({ "Description": "bulk-updated" }))
    .await?;
```

## See also

- Auth setup: [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- Retries & errors: [Operations](../03-operations.md)
- Integration test (batch usage): [`tests/composite_batch.rs`](../../../crates/force/tests/composite_batch.rs)
- Example: [`soql_mass_op.rs`](../../../crates/force/examples/soql_mass_op.rs)
- Rustdoc: `cargo doc --no-deps --features composite --open` → `force::api::composite`
