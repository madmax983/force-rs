//! Criterion benchmarks for the SOAP hot paths touched by the perf sweep.
//!
//! Run with:
//! ```bash
//! cargo bench -p force --features soap,bench-internals --bench soap_bench
//! ```
//!
//! These compile against both the pre-optimization and post-optimization
//! versions of the benched functions (their signatures are stable across the
//! change; `escape_text` is consumed via `.len()`, which works for both a
//! `String` and a `Cow<'_, str>` return).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use force::api::soap::{SObject, bench_hooks};

/// Builds a realistic ~50-record SOAP `queryResponse` body (no fault).
fn query_success_body(records: usize) -> String {
    let mut body = String::with_capacity(records * 256);
    body.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?><soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns="urn:partner.soap.sforce.com" xmlns:sf="urn:sobject.partner.soap.sforce.com"><soapenv:Body><queryResponse><result><done>true</done><queryLocator xsi:nil="true"/><records>"#,
    );
    for i in 0..records {
        body.push_str("<sf:type>Account</sf:type>");
        body.push_str("<sf:Id>001xx00000000");
        body.push_str(&i.to_string());
        body.push_str("AAA</sf:Id><sf:Name>Account Number ");
        body.push_str(&i.to_string());
        body.push_str("</sf:Name><sf:Industry>Technology</sf:Industry>");
    }
    body.push_str("</records><size>");
    body.push_str(&records.to_string());
    body.push_str("</size></result></queryResponse></soapenv:Body></soapenv:Envelope>");
    body
}

/// Builds a slice of `n` records, each carrying `fields` string fields.
fn typed_records(n: usize, fields: usize) -> Vec<SObject> {
    (0..n)
        .map(|r| {
            let mut obj = SObject::new("Account");
            for f in 0..fields {
                obj.set_field(format!("Field{f}"), format!("value-{r}-{f}"));
            }
            obj
        })
        .collect()
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct WideRecord {
    #[serde(rename = "Field0")]
    field0: String,
    #[serde(rename = "Field1")]
    field1: String,
    #[serde(rename = "Field2")]
    field2: String,
    #[serde(rename = "Field3")]
    field3: String,
    #[serde(rename = "Field4")]
    field4: String,
    #[serde(rename = "Field5")]
    field5: String,
    #[serde(rename = "Field6")]
    field6: String,
    #[serde(rename = "Field7")]
    field7: String,
    #[serde(rename = "Field8")]
    field8: String,
    #[serde(rename = "Field9")]
    field9: String,
}

fn bench_escape_text(c: &mut Criterion) {
    let clean = "001xx0000000001AAA";
    let dirty = "Acme <Corp> & \"Sons\" 'Ltd' > test & more <b>bold</b>";

    c.bench_function("escape_text/no_escape", |b| {
        b.iter(|| black_box(bench_hooks::escape_text(black_box(clean)).len()));
    });
    c.bench_function("escape_text/needs_escape", |b| {
        b.iter(|| black_box(bench_hooks::escape_text(black_box(dirty)).len()));
    });
}

fn bench_parse_fault(c: &mut Criterion) {
    let body = query_success_body(50);
    c.bench_function("parse_fault/query_success_50", |b| {
        b.iter(|| black_box(bench_hooks::parse_fault(black_box(&body))));
    });
}

fn bench_records_to_typed(c: &mut Criterion) {
    let records = typed_records(50, 10);
    c.bench_function("records_to_typed/50x10", |b| {
        b.iter(|| {
            let out: Vec<WideRecord> = bench_hooks::records_to_typed(black_box(&records)).unwrap();
            black_box(out.len())
        });
    });
}

criterion_group!(
    benches,
    bench_escape_text,
    bench_parse_fault,
    bench_records_to_typed
);
criterion_main!(benches);
