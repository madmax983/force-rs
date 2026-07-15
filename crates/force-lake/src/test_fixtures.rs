//! JSON fixtures for constructing [`force::types::describe::SObjectDescribe`]
//! values in unit tests.
//!
//! `force`'s in-crate mock describe builders live behind a `#[cfg(test)]`
//! `pub(crate)` module and are not reachable across crates, so `force-lake`
//! builds describe values by deserializing hand-assembled JSON. Serde treats
//! absent `Option` fields as `None`, so only the non-optional scalar fields need
//! to be populated here.

#![cfg(test)]
#![allow(clippy::needless_pass_by_value)]

use serde_json::{Value, json};

/// Builds a `fieldDescribe` JSON object with all required (non-`Option`) keys,
/// merging any per-field overrides (e.g. `nillable`, `precision`, `scale`).
pub(crate) fn field_json(name: &str, sf_type: &str, overrides: Value) -> Value {
    let mut field = json!({
        "aggregatable": true,
        "autoNumber": false,
        "byteLength": 255,
        "calculated": false,
        "cascadeDelete": false,
        "caseSensitive": false,
        "createable": true,
        "custom": false,
        "defaultedOnCreate": false,
        "dependentPicklist": false,
        "deprecatedAndHidden": false,
        "digits": 0,
        "displayLocationInDecimal": false,
        "encrypted": false,
        "externalId": false,
        "filterable": true,
        "groupable": true,
        "highScaleNumber": false,
        "htmlFormatted": false,
        "idLookup": name == "Id",
        "label": name,
        "length": 255,
        "name": name,
        "nameField": name == "Name",
        "namePointing": false,
        "nillable": true,
        "permissionable": true,
        "polymorphicForeignKey": false,
        "precision": 0,
        "queryByDistance": false,
        "referenceTo": [],
        "restrictedDelete": false,
        "restrictedPicklist": false,
        "scale": 0,
        "soapType": "xsd:string",
        "sortable": true,
        "type": sf_type,
        "unique": false,
        "updateable": true,
        "writeRequiresMasterRead": false,
    });

    if let (Some(target), Some(over)) = (field.as_object_mut(), overrides.as_object()) {
        for (key, value) in over {
            target.insert(key.clone(), value.clone());
        }
    }
    field
}

/// Builds a full `sObjectDescribe` JSON object with the given fields.
pub(crate) fn describe_json(name: &str, fields: Vec<Value>) -> Value {
    json!({
        "activateable": false,
        "createable": true,
        "custom": false,
        "customSetting": false,
        "deletable": true,
        "deprecatedAndHidden": false,
        "feedEnabled": false,
        "hasSubtypes": false,
        "isSubtype": false,
        "label": name,
        "labelPlural": name,
        "layoutable": true,
        "mergeable": false,
        "mruEnabled": true,
        "name": name,
        "queryable": true,
        "replicateable": true,
        "retrieveable": true,
        "searchable": true,
        "triggerable": true,
        "undeletable": true,
        "updateable": true,
        "fields": fields,
        "childRelationships": [],
        "recordTypeInfos": [],
        "urls": {},
    })
}
