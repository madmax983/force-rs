//! One-shot profiling harness for `force_lake::build_record_batch`
//! (Arrow `RecordBatch` assembly from Salesforce JSON records).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `build_record_batch` entry point, meant to be executed once under
//! `valgrind --tool=callgrind` (instruction counts) and `valgrind --tool=dhat`
//! (allocation counts) so those tools attribute cost to real call stacks
//! instead of a criterion harness loop.
//!
//! Workload: `SnapshotSink::snapshot` (crates/force-lake/src/snapshot.rs)
//! calls `build_record_batch` once per `chunk` of `records.chunks(batch_size)`,
//! where `batch_size` defaults to `LakeConfig::DEFAULT_BATCH_SIZE` (10,000
//! records) -- exactly what one chunk of a full-object snapshot sink run
//! assembles into a `RecordBatch` before Parquet encoding. This harness
//! reproduces that call directly: `ROW_COUNT` rows of a wide (~15-field,
//! mixed-type) Opportunity-shaped schema, matching the field mix exercised by
//! `crates/force-lake/benches/record_batch_bench.rs` (two `Decimal128`
//! currency fields, two timestamp fields, a date field, string/int/bool
//! fields), scaled up to the real per-chunk row count. Field values are
//! generated from the row index with no RNG, so repeated runs are
//! byte-for-byte deterministic, which is required for callgrind/dhat
//! comparisons to be meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force-lake --bench record_batch_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force-lake --bench record_batch_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'record_batch_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    missing_docs
)]

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use force_lake::build_record_batch;
use serde_json::{Value, json};

/// Rows assembled in one profiling run -- the real per-chunk row count
/// (`LakeConfig::DEFAULT_BATCH_SIZE`) that `SnapshotSink::snapshot` passes to
/// `build_record_batch` for every chunk of a full-object snapshot.
const ROW_COUNT: usize = 10_000;

/// A 15-field Opportunity-shaped schema: a mix of Salesforce-owned scalar
/// types (string, int, float, two currency decimals, two booleans, a date,
/// two timestamps), matching the field mix a real snapshot sink run would
/// derive from `force::schema::generate_iceberg_schema` for a wide custom
/// object.
fn wide_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("Id", DataType::Utf8, false),
        Field::new("Name", DataType::Utf8, true),
        Field::new("Description", DataType::Utf8, true),
        Field::new("Industry", DataType::Utf8, true),
        Field::new("Website", DataType::Utf8, true),
        Field::new("Employees", DataType::Int32, true),
        Field::new("AnnualRevenue", DataType::Float64, true),
        Field::new("Amount", DataType::Decimal128(18, 2), true),
        Field::new("Discount", DataType::Decimal128(9, 4), true),
        Field::new("IsActive", DataType::Boolean, true),
        Field::new("IsWon", DataType::Boolean, true),
        Field::new("CloseDate", DataType::Date32, true),
        Field::new(
            "CreatedDate",
            DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
            true,
        ),
        Field::new(
            "LastModifiedDate",
            DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
            true,
        ),
        Field::new("ExternalId", DataType::Utf8, true),
    ]))
}

fn wide_records(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({
                "Id": format!("001xx{i:013}"),
                "Name": format!("Account {i}"),
                "Description": "A reasonably long-ish description field for column sizing.",
                "Industry": "Technology",
                "Website": format!("https://example-{i}.com"),
                "Employees": (i % 5000) as i64,
                "AnnualRevenue": (i as f64) * 1234.5,
                "Amount": format!("{}.{:02}", i, i % 100),
                "Discount": format!("0.{:04}", i % 10000),
                "IsActive": i % 2 == 0,
                "IsWon": i % 3 == 0,
                "CloseDate": "2024-01-15",
                "CreatedDate": "2024-01-15T10:30:00.000+0000",
                "LastModifiedDate": "2024-06-30T23:59:59.500Z",
                "ExternalId": format!("EXT-{i}"),
            })
        })
        .collect()
}

fn main() {
    let schema = wide_schema();
    let records = wide_records(ROW_COUNT);

    let batch = build_record_batch(&schema, &records).expect("batch builds");

    // Keep the result observable so the compiler can't fold the whole run away.
    println!(
        "row_count={ROW_COUNT} batch_rows={} batch_columns={}",
        batch.num_rows(),
        batch.num_columns()
    );
}
