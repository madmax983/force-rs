# Data Utility

Test-data helpers layered on schema metadata: generate mock records from an
`SObjectDescribe`, seed hundreds of them into an org, mask PII before it
leaves an org, validate records before you send them, and export query
results to local JSONL. These are free functions and structs in
`force::data`, not a `client.xxx()` handler — each one takes either a
`&SObjectDescribe` or a `&ForceClient` directly.

- **Feature flag:** `data_utility` (depends on `composite`)
- **Accessor:** `force::data::{generate_mock_record, DataSeeder, DataMasker, DataValidator, DataArchiver}`
- **Status:** Preview — the API may still change before stabilization.

```toml
force = { version = "...", features = ["data_utility"] }
```

## Methods

### `generate_mock_record` / `generate_mock_query` — mock data from a describe

Generates a schema-compliant `DynamicSObject` for any `createable` field,
skipping `autoNumber`/`calculated` fields and picking a valid picklist value
where one exists.

```rust
// Fetch the metadata for an Account
let describe = client.rest().describe("Account").await?;

// Generate a fake record that conforms to the schema
let fake_account = force::data::generate_mock_record(&describe);
```

### `DataSeeder` — generate + insert via Composite Batch

Combines `generate_mock_record` with the Composite Batch API to insert many
records in one round-trip, chunking automatically at the batch-size limit.

```rust
let seeder = force::data::DataSeeder::new(&client).halt_on_error(true);

// Generate and insert 500 mock Accounts via the Composite Batch API.
let inserted = seeder.seed("Account", 500).await?;
println!("Seeded {inserted} Accounts");
```

### `DataMasker` — redact PII before it leaves an org

Uses the object's describe metadata (field type, name heuristics) to redact
sensitive fields (email, phone, SSN-shaped strings, ...) on a record already
in memory.

```rust
let describe = client.rest().describe("Contact").await?;
let masker = force::data::DataMasker::new(&describe);

masker.mask_record(&mut contact); // contact: DynamicSObject
println!("{:?}", contact.get_field_as::<String>("Email")); // Some("***@***.***")
```

### `DataValidator` — client-side validation before you send a record

Checks a record against the describe's required/length/type constraints so
invalid payloads fail locally instead of burning an API call.

```rust
let describe = client.rest().describe("Contact").await?;
let validator = force::data::DataValidator::new(&describe);

if let Err(errors) = validator.validate(&contact) {
    for err in errors {
        println!("Validation Error: {err}");
    }
}
```

### `DataArchiver` — export query results to local JSONL

Streams a SOQL query straight to a JSONL file, optionally masking each record
with `DataMasker` on the way out (`export_masked_to_jsonl` fetches the
describe for you).

```rust
let archiver = force::data::DataArchiver::new(&client);

// Plain export
let count = archiver
    .export_to_jsonl::<serde_json::Value>("SELECT Id, Name FROM Account", "accounts.jsonl")
    .await?;

// PII-masked export, driven by the Contact describe metadata
let count = archiver
    .export_masked_to_jsonl("Contact", "SELECT Id, Name, Email FROM Contact", "contacts.jsonl")
    .await?;
```

## See also

- Rustdoc: `cargo doc --no-deps --features data_utility --open` → `force::data`
- [Composite](composite.md) — the Batch API `DataSeeder` builds on
