# SOAP Partner API

> **Status:** this surface ships in **PR #1205** (branch `soap-api`), **not yet merged** to
> `trunk-dev` as of this writing. Feature gate `soap`; design in
> [ADR-032](../../adr/032-soap-api-design.md).

Classic untyped SOAP **Partner** API: generic records (no per-org WSDL codegen), CRUD, query,
search, describe, and utility calls. There is no SOAP `login()` — the handler reuses the client's
existing OAuth token in the SOAP `SessionHeader`.

## Feature gate

```toml
[dependencies]
force = { version = "*", features = ["soap"] }   # soap = ["dep:quick-xml"]
```

`soap` is included in the `full` (and `all`) aggregate. XML is parsed with `quick-xml` 0.41
(event-based, no serde-derive, rustls-only posture).

Endpoint: `{instance_url}/services/Soap/u/{version}` (the leading `v` is stripped, e.g. `v67.0`
→ `67.0`).

## Accessor & config

```rust
let soap = client.soap();                         // owned SoapHandler<A>, cheap to Clone

// Override the CallOptions client id (default "force-rs"), or omit the header entirely:
let soap = client.soap().with_client(Some("my-app"));
let soap = client.soap().with_client(None::<String>);
```

## Methods

All return `force::error::Result<...>`. CRUD calls accept up to 200 records; results are
positional (one per input, in order).

### CRUD

| Method | Returns |
| --- | --- |
| `create(&[SObject])` | `Vec<SaveResult>` |
| `update(&[SObject])` | `Vec<SaveResult>` |
| `upsert(external_id_field, &[SObject])` | `Vec<UpsertResult>` |
| `delete(&[S: AsRef<str>])` | `Vec<DeleteResult>` |
| `retrieve(sobject_type, &[fields], &[ids])` | `Vec<SObject>` (not-found Ids omitted) |

### Query & search

| Method | Returns |
| --- | --- |
| `query(soql)` | `QueryResult` |
| `query_more(query_locator)` | `QueryResult` |
| `query_all(soql)` | `QueryResult` (includes soft-deleted / archived) |
| `search(sosl)` | `SearchResult` |

When `QueryResult::done == false`, pass `QueryResult::query_locator` to `query_more`.

### Describe / metadata

| Method | Returns |
| --- | --- |
| `describe_sobject(sobject_type)` | `DescribeSObjectResult` |
| `describe_sobjects(&[sobject_types])` | `Vec<DescribeSObjectResult>` (≤ 100 per call) |
| `describe_global()` | `DescribeGlobalResult` |

### Misc utilities

| Method | Returns |
| --- | --- |
| `get_user_info()` | `UserInfo` |
| `get_server_timestamp()` | `chrono::DateTime<chrono::Utc>` |

### Typed convenience layer

Bridges serde structs ↔ the generic `SObject` field bag; delegates to the untyped calls. The
Partner API returns every field as a **string** — target `T` fields should be `String` /
`Option<String>` (or use `#[serde(deserialize_with = "…")]`).

| Method | Returns |
| --- | --- |
| `query_typed::<T>(soql)` | `Vec<T>` (auto-follows `queryMore`, all pages) |
| `query_typed_page::<T>(soql)` | `(Vec<T>, done, next_locator)` |
| `query_more_typed_page::<T>(query_locator)` | `(Vec<T>, done, next_locator)` |
| `retrieve_typed::<T>(sobject_type, &[fields], &[ids])` | `Vec<Option<T>>` (positional; `None` for not-found) |
| `create_typed::<T>(sobject_type, &[records])` | `Vec<SaveResult>` |
| `update_typed::<T>(sobject_type, &[records])` | `Vec<SaveResult>` |
| `upsert_typed::<T>(sobject_type, external_id_field, &[records])` | `Vec<UpsertResult>` |

> `retrieve_typed` differs from untyped `retrieve`: it is positional (one entry per requested Id,
> `None` for not-found) rather than dropping missing records.

## The `SObject` builder

```rust
use force::api::soap::SObject;

let record = SObject::new("Contact")
    .with_field("LastName", "Doe")
    .with_field("Email", "jane@example.com");

// Null out a field on update:
let update = SObject::new("Account")
    .with_field("Id", "001...")
    .with_null_field("Description");
```

Read fields back with `record.get("LastName")` (first non-null match) or `record.id()`
(shortcut for `get("Id")`). `SObject` also has `set_field(...)` for mutation.

## Usage

```rust
use force::api::soap::SObject;

// Utility + describe (zero side effects)
let _ts   = client.soap().get_server_timestamp().await?;   // DateTime<Utc>
let info  = client.soap().get_user_info().await?;          // info.user_id, info.organization_id
let acct  = client.soap().describe_sobject("Account").await?;

// CRUD round-trip
let record = SObject::new("Contact").with_field("LastName", "Doe");
let saves  = client.soap().create(&[record]).await?;
let id     = saves[0].id.clone().expect("create returned an Id");

let retrieved = client.soap().retrieve("Contact", &["Id", "LastName"], &[&id]).await?;

// Typed query — auto-follows queryMore, returns Vec<T>
#[derive(serde::Deserialize)]
struct Contact {
    #[serde(rename = "Id")]       id: String,
    #[serde(rename = "LastName")] last_name: String,
}
let soql = format!("SELECT Id, LastName FROM Contact WHERE Id = '{id}'");
let typed: Vec<Contact> = client.soap().query_typed(&soql).await?;

let _ = client.soap().delete(&[id]).await;                 // Vec<DeleteResult>
```

Manual pagination:

```rust
let mut page = client.soap().query("SELECT Id, Name FROM Account").await?;
for r in &page.records { println!("{:?}", r.get("Name")); }
while !page.done {
    let Some(loc) = page.query_locator.as_deref() else { break; };
    page = client.soap().query_more(loc).await?;
}
```

## Error handling

Two distinct failure channels:

- **Whole-call faults → `Err(ForceError::Soap(SoapFault))`.** A transport-level SOAP fault
  (malformed query, invalid session, XML parse failure) surfaces as `ForceError::Soap`.
  `SoapFault` carries `fault_code`, `fault_string`, and optional `exception_code`; use
  `fault.is_invalid_session()` to detect `INVALID_SESSION_ID`.
- **Per-record failures are NOT errors.** A partially-failed `create`/`update`/`upsert`/`delete`
  returns `Ok(Vec<...Result>)` on an HTTP 200; a failed row has `success == false` and a
  populated `errors: Vec<SoapError>` (`status_code`, `message`, `fields`). Always inspect each
  result, not just the outer `Result`.

```rust
match client.soap().create(&records).await {
    Ok(results) => {
        for r in results {
            if r.success {
                println!("created {:?}", r.id);
            } else {
                for e in &r.errors {
                    eprintln!("{}: {} {:?}", e.status_code, e.message, e.fields);
                }
            }
        }
    }
    Err(force::error::ForceError::Soap(fault)) if fault.is_invalid_session() => {
        // handled automatically: the handler force-refreshes and retries once
    }
    Err(e) => return Err(e),
}
```

An expired session surfaces as an HTTP 500 fault (not a 401), so the shared 401-refresh
middleware never fires. The handler detects `INVALID_SESSION_ID`, force-refreshes the token, and
retries **exactly once**; a second session fault returns `ForceError::Soap`.

## Not in v1

Nested relationship records / child subqueries (flat record model), `merge`, `convertLead`,
`setPassword`, streaming `queryMore` ergonomics, and `LimitInfoHeader` surfacing are documented
follow-ups. See [ADR-032](../../adr/032-soap-api-design.md).
