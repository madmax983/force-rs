//! One-shot profiling harness for `force::data::generate_mock_record`
//! (schema-driven synthetic record generation).
//!
//! This is not a criterion benchmark: it is a single realistic run of the
//! public `generate_mock_record` entry point, meant to be executed once
//! under `valgrind --tool=callgrind` (instruction counts) and
//! `valgrind --tool=dhat` (allocation counts) so those tools attribute cost
//! to real call stacks instead of a criterion harness loop.
//!
//! Workload: generate 20,000 mock records against a single custom-object-shaped
//! describe with 25 fields spanning every `FieldType` variant the generator
//! branches on, including the four "skip, no mock value" types (`Base64`,
//! `Datacategorygroupreference`, `Location`, `Address`) and three
//! not-createable/system fields (a read-only field, an autonumber field, and a
//! formula field) that real org describes always carry alongside creatable
//! ones. This is the same public path a caller uses in the documented
//! "populating sandboxes with dummy data" use case: describe an object once,
//! then call `generate_mock_record` once per row to fabricate a batch of test
//! records. Field count, types, and iteration count are fixed (no RNG) so
//! repeated runs are byte-for-byte deterministic, which is required for
//! callgrind/dhat comparisons to be meaningful.
//!
//! Run directly (`cargo run` doesn't support `--bench`, so use `cargo bench`):
//! ```bash
//! cargo bench -p force --features data_utility --bench data_faker_profile
//! ```
//!
//! Profile (this is a `[[bench]]` target, not a `[[bin]]`: the compiled
//! executable lands under `target/release/deps/`, not `target/release/`):
//! ```bash
//! CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p force --features data_utility --bench data_faker_profile
//! BIN=$(find target/release/deps -maxdepth 1 -name 'data_faker_profile-*' -executable -not -name '*.d')
//! valgrind --tool=callgrind --callgrind-out-file=callgrind.out "$BIN"
//! callgrind_annotate callgrind.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out "$BIN"
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use force::data::generate_mock_record;
use force::types::SObjectDescribe;

/// Number of mock records generated in one profiling run.
const RECORD_COUNT: usize = 20_000;

#[allow(clippy::too_many_arguments)]
fn field(
    name: &str,
    ty: &str,
    label: &str,
    createable: bool,
    auto_number: bool,
    calculated: bool,
    ref_to: &[&str],
    picklist_values: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut v = serde_json::json!({
        "name": name, "type": ty, "label": label, "createable": createable,
        "autoNumber": auto_number, "calculated": calculated,
        "aggregatable": true, "byteLength": 255,
        "cascadeDelete": false, "caseSensitive": false, "custom": true,
        "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
        "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
        "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
        "idLookup": false, "length": 255, "nameField": false, "namePointing": false, "nillable": true,
        "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
        "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
        "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false,
        "referenceTo": ref_to
    });
    if let Some(pv) = picklist_values {
        v["picklistValues"] = pv;
    }
    v
}

fn picklist_values() -> serde_json::Value {
    serde_json::json!([
        { "active": false, "defaultValue": false, "label": "Legacy", "value": "Legacy" },
        { "active": true, "defaultValue": false, "label": "Technology", "value": "Technology" },
        { "active": true, "defaultValue": true, "label": "Finance", "value": "Finance" },
    ])
}

/// A 25-field custom-object-shaped describe covering every `FieldType`
/// variant `generate_mock_record` branches on, plus the not-createable /
/// autonumber / formula fields every real describe also carries.
fn custom_object_describe() -> SObjectDescribe {
    let describe_json = serde_json::json!({
        "name": "Case_Record__c", "label": "Case Record", "custom": true, "queryable": true,
        "activateable": false, "createable": true, "customSetting": false, "deletable": true,
        "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
        "isSubtype": false, "keyPrefix": "a01", "labelPlural": "Case Records", "layoutable": true,
        "mergeable": false, "mruEnabled": true, "replicateable": true, "retrieveable": true,
        "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
        "urls": {}, "childRelationships": [], "recordTypeInfos": [],
        "fields": [
            field("Id", "id", "Record ID", false, false, false, &[], None),
            field("Name", "string", "Record Name", true, false, false, &[], None),
            field("Description__c", "textarea", "Description", true, false, false, &[], None),
            field("SecretNote__c", "encryptedstring", "Secret Note", true, false, false, &[], None),
            field("EmployeeCount__c", "int", "Employee Count", true, false, false, &[], None),
            field("Revenue__c", "double", "Revenue", true, false, false, &[], None),
            field("Discount__c", "percent", "Discount", true, false, false, &[], None),
            field("Budget__c", "currency", "Budget", true, false, false, &[], None),
            field("IsActive__c", "boolean", "Is Active", true, false, false, &[], None),
            field("StartDate__c", "date", "Start Date", true, false, false, &[], None),
            field("CheckInTime__c", "datetime", "Check-In Time", true, false, false, &[], None),
            field("DailyAlarm__c", "time", "Daily Alarm", true, false, false, &[], None),
            field("ContactEmail__c", "email", "Contact Email", true, false, false, &[], None),
            field("ContactPhone__c", "phone", "Contact Phone", true, false, false, &[], None),
            field("Website__c", "url", "Website", true, false, false, &[], None),
            field(
                "Industry__c",
                "picklist",
                "Industry",
                true,
                false,
                false,
                &[],
                Some(picklist_values()),
            ),
            field("Tags__c", "multipicklist", "Tags", true, false, false, &[], None),
            field("Language__c", "combobox", "Preferred Language", true, false, false, &[], None),
            field("ParentId", "reference", "Parent", true, false, false, &["Account"], None),
            field("Category__c", "anyType", "Category", true, false, false, &[], None),
            field("Attachment__c", "base64", "Attachment", true, false, false, &[], None),
            field(
                "DataCategory__c",
                "datacategorygroupreference",
                "Data Category",
                true,
                false,
                false,
                &[],
                None,
            ),
            field("Coordinates__c", "location", "Coordinates", true, false, false, &[], None),
            field("MailingAddress__c", "address", "Mailing Address", true, false, false, &[], None),
            field("ReadOnly__c", "string", "Read Only", false, false, false, &[], None),
            field("AutoNum__c", "string", "Auto Number", true, true, false, &[], None),
            field("Formula__c", "string", "Formula", true, false, true, &[], None),
        ]
    });
    serde_json::from_value(describe_json).expect("valid custom object describe")
}

fn main() {
    let describe = custom_object_describe();

    let mut total_fields = 0usize;
    for _ in 0..RECORD_COUNT {
        let record = generate_mock_record(&describe);
        total_fields += record.field_count();
    }

    // Keep the result observable so the compiler can't fold the whole run away.
    println!("record_count={RECORD_COUNT} total_fields={total_fields}");
}
