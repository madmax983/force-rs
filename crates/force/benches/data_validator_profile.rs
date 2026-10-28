//! One-shot profiling harness for `DataValidator::validate`
//! (schema-driven client-side record validation).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `DataValidator::validate` entry point, meant to be executed once
//! under `valgrind --tool=callgrind` (instruction counts) so that tool
//! attributes cost to real call stacks instead of a criterion harness loop.
//!
//! Workload: describe a 200-field custom object (a realistic shape for a
//! heavily-customized org object -- core standard objects routinely
//! accumulate 100-300+ custom fields over years of admin/declarative
//! changes), generate 20,000 populated records against that describe with
//! the existing `generate_mock_record` helper, and run every record through
//! `DataValidator::validate` with a single validator built once and reused
//! across the whole batch -- the same public path a caller uses in the
//! documented "validate a batch before a bulk upsert" use case (see the
//! module's own doc example: describe once, build one `DataValidator`, then
//! call `.validate()` per record). Field count, record count, and field
//! shapes are fixed (no RNG) so repeated runs are byte-for-byte
//! deterministic, which is required for callgrind comparisons to be
//! meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features data_utility --bench data_validator_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features data_utility --bench data_validator_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'data_validator_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::data::{DataValidator, generate_mock_record};
use force::types::SObjectDescribe;

/// Number of createable fields on the profiled describe -- a realistic
/// field count for a heavily-customized core object in a mature org.
const FIELD_COUNT: usize = 200;

/// Number of records validated in one profiling run.
const RECORD_COUNT: usize = 20_000;

/// The `FieldType` values cycled through while building the wide describe,
/// covering every branch `DataValidator::validate` and `generate_mock_record`
/// switch on.
const FIELD_TYPES: &[&str] = &[
    "string", "textarea", "email", "phone", "url", "picklist", "int", "double", "currency",
    "percent", "boolean", "date", "datetime", "time",
];

#[allow(clippy::too_many_arguments)]
fn field(name: &str, ty: &str, label: &str, nillable: bool, length: i32) -> serde_json::Value {
    serde_json::json!({
        "name": name, "type": ty, "label": label, "createable": true,
        "autoNumber": false, "calculated": false,
        "aggregatable": true, "byteLength": length * 3,
        "cascadeDelete": false, "caseSensitive": false, "custom": true,
        "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
        "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
        "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
        "idLookup": false, "length": length, "nameField": false, "namePointing": false,
        "nillable": nillable, "permissionable": false, "polymorphicForeignKey": false,
        "precision": 0, "queryByDistance": false, "restrictedDelete": false,
        "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
        "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
        "referenceTo": []
    })
}

/// A `FIELD_COUNT`-field custom-object-shaped describe: every field cycles
/// through `FIELD_TYPES` so the record populated from it exercises every
/// validation branch (string length, boolean, numeric) many times over, the
/// same way a wide real-world custom object does.
fn wide_describe() -> SObjectDescribe {
    let fields: Vec<serde_json::Value> = (0..FIELD_COUNT)
        .map(|i| {
            let ty = FIELD_TYPES[i % FIELD_TYPES.len()];
            let name = format!("Field_{i}__c");
            let label = format!("Field {i}");
            // Every 10th string-like field gets a short max length so the
            // length-exceeded branch in `validate` is actually exercised,
            // matching how real describes mix short and long text fields.
            let length = if matches!(ty, "string" | "textarea" | "email" | "phone" | "url") {
                if i % 10 == 0 { 5 } else { 255 }
            } else {
                0
            };
            field(&name, ty, &label, true, length)
        })
        .collect();

    let describe_json = serde_json::json!({
        "name": "Wide_Object__c", "label": "Wide Object", "custom": true, "queryable": true,
        "activateable": false, "createable": true, "customSetting": false, "deletable": true,
        "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
        "isSubtype": false, "keyPrefix": "a02", "labelPlural": "Wide Objects", "layoutable": true,
        "mergeable": false, "mruEnabled": true, "replicateable": true, "retrieveable": true,
        "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
        "urls": {}, "childRelationships": [], "recordTypeInfos": [],
        "fields": fields,
    });
    serde_json::from_value(describe_json).expect("valid wide describe")
}

fn main() {
    let describe = wide_describe();
    let validator = DataValidator::new(&describe);

    let mut error_count = 0usize;
    for _ in 0..RECORD_COUNT {
        let record = generate_mock_record(&describe);
        if let Err(errors) = validator.validate(&record) {
            error_count += errors.len();
        }
    }

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("record_count={RECORD_COUNT} field_count={FIELD_COUNT} error_count={error_count}");
}
