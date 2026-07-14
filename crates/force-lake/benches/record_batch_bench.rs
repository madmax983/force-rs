//! Criterion benchmark for Arrow [`RecordBatch`] assembly.
//!
//! Run with:
//! ```bash
//! cargo bench -p force-lake --bench record_batch_bench
//! ```
//!
//! Measures `build_record_batch` over 1000 rows of a wide (~15-field, mixed
//! type) object. The function signature is stable across the perf sweep, so
//! this compiles against both the pre- and post-optimization versions.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    missing_docs
)]

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use force_lake::build_record_batch;
use serde_json::{Value, json};

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

fn bench_build_record_batch(c: &mut Criterion) {
    let schema = wide_schema();
    let records = wide_records(1000);
    c.bench_function("build_record_batch/1000x15", |b| {
        b.iter(|| {
            let batch = build_record_batch(black_box(&schema), black_box(&records)).unwrap();
            black_box(batch.num_rows())
        });
    });
}

criterion_group!(benches, bench_build_record_batch);
criterion_main!(benches);
