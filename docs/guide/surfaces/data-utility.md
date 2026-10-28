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
records, chunking automatically into batches of 25 (the Composite Batch
limit) — one round-trip per chunk, not one round-trip total.
`seed()` returns only a count of successful inserts: a failed record is
dropped silently unless `halt_on_error(true)`, in which case the whole call
fails with one generic error and no per-record detail (open gap, see
[the vantage spec](../../vantage/data-seeder.md)).

```rust
let seeder = force::data::DataSeeder::new(&client).halt_on_error(true);

// Generate and insert mock records for a custom object with no createable
// Reference fields, via the Composite Batch API, 25 at a time.
let inserted = seeder.seed("Mock_Fixture__c", 100).await?;
println!("Seeded {inserted} Mock_Fixture__c records");
```

> **Caveat:** `generate_mock_record` fills every createable `Reference`
> field with a placeholder string, not a valid Salesforce Id — Salesforce
> rejects that value. Most standard objects have at least one createable
> Reference field (e.g. `OwnerId`), so seeding them with an unmodified
> `seed()` call fails today; this works cleanly only for objects with no
> createable Reference fields, such as a purpose-built custom object, or
> after you post-process the generated records to drop/populate those
> fields yourself.

### `DataMasker` — redact PII before it leaves an org

Redacts a field if its describe metadata says it's `encrypted`, its
`FieldType` is `Email`/`Phone`, or its API name contains `ssn`, `password`,
`creditcard`, or `secret` (case-insensitive). This is a name/type heuristic,
not content inspection — a field like `TaxIdentifier__c` holding an
SSN-shaped value is **not** masked because neither its name nor its type
matches; rename or retype the field, or mask it yourself, if you rely on
this for compliance.

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
