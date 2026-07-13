# Bulk API 2.0

High-volume ingest and query over `/services/data/vXX.0/jobs/`. The handler
manages the full job lifecycle for you — create job, upload CSV, close, poll to
completion — and streams query results back. Use it for thousands to millions of
records where the REST API would burn call limits.

- **Feature flag:** `bulk` (pulls in `dep:csv` for CSV encode/decode)
- **Accessor:** `client.bulk()` → `BulkHandler`

```toml
force = { version = "...", features = ["bulk"] }
```

## Ingest (insert / update / delete)

`insert` and `update` take any `&[T] where T: Serialize`; each drives a job to
terminal state and returns the final `JobInfo`.

```rust
#[derive(serde::Serialize)]
struct Account { #[serde(rename = "Name")] name: String }

let accounts = vec![Account { name: "Acme".into() }];

let job = client.bulk().insert("Account", &accounts).await?;
println!("{:?}: {} processed, {} failed",
    job.state, job.number_records_processed.unwrap_or(0), job.number_records_failed.unwrap_or(0));

client.bulk().update("Account", &records).await?;
client.bulk().delete("Account", &["001...".to_string()]).await?; // delete by IDs
```

## Query (streaming)

`query::<T>` polls the query job, then hands back a stream of typed records:

```rust
use futures::StreamExt;

let stream = client.bulk().query::<Account>("SELECT Id, Name FROM Account").await?;
let stream = stream.into_stream();
let mut stream = std::pin::pin!(stream);

while let Some(row) = stream.next().await {
    let account = row?;
    // ...
}
```

Lower-level job control is available too: `update_job`, `delete_job`,
`query_results`, `delete_query_job`, plus `BulkPollPolicy` to tune polling.

## See also

- Auth setup: [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- Retries, errors, polling: [Operations](../03-operations.md)
- Examples:
  [`bulk_insert.rs`](../../../crates/force/examples/bulk_insert.rs),
  [`bulk_query.rs`](../../../crates/force/examples/bulk_query.rs),
  [`bulk_update.rs`](../../../crates/force/examples/bulk_update.rs),
  [`bulk_delete.rs`](../../../crates/force/examples/bulk_delete.rs),
  [`bulk_error_handling.rs`](../../../crates/force/examples/bulk_error_handling.rs),
  [`soql_mass_op.rs`](../../../crates/force/examples/soql_mass_op.rs)
- Rustdoc: `cargo doc --no-deps --features bulk --open` → `force::api::bulk`
