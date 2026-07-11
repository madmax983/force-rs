//! DBML generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate DBML schema from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates a DBML representation from an SObject describe result.
pub fn generate_dbml(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_dbml(&mut out, describe);
    out
}

/// Writes a DBML representation from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_dbml(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "Table {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in &fields {
        let dbml_type = map_type(&field.type_);
        let _ = write!(out, "  {} {}", field.name, dbml_type);

        let mut has_setting = false;
        let mut write_setting_separator = |out: &mut String| {
            if has_setting {
                out.push_str(", ");
            } else {
                out.push_str(" [");
                has_setting = true;
            }
        };

        if field.name == "Id" {
            write_setting_separator(out);
            out.push_str("pk");
        }

        if !field.nillable && field.name != "Id" {
            write_setting_separator(out);
            out.push_str("not null");
        }

        let label = &field.label;
        write_setting_separator(out);
        let _ = write!(out, "note: '{}'", label.replace('\'', "''"));

        if has_setting {
            out.push(']');
        }

        let _ = writeln!(out);
    }

    let _ = writeln!(out, "}}\n");

    // ⚡ Bolt: Write relationships directly to the output buffer instead of allocating a `Vec<String>`.
    for field in &fields {
        if matches!(field.type_, FieldType::Reference) && !field.reference_to.is_empty() {
            for ref_target in &field.reference_to {
                let _ = writeln!(
                    out,
                    "Ref: {}.{} > {}.Id",
                    describe.name, field.name, ref_target
                );
            }
        }
    }
}

fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int => "int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "decimal",
        FieldType::Date => "date",
        FieldType::Datetime => "timestamp",
        FieldType::Time => "time",
        FieldType::Id | FieldType::Reference => "varchar(18)",
        FieldType::Textarea => "text",
        FieldType::Base64 => "blob",
        _ => "varchar",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use crate::types::describe::SObjectDescribe;

    #[test]
    fn test_generate_dbml() {
        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
            "name": "Account",
            "label": "Account Object",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Account ID", "createable": false,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": true, "referenceTo": [],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                },
                {
                    "name": "ParentId", "type": "reference", "label": "Parent Account ID", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": true,
                    "defaultedOnCreate": false, "referenceTo": ["Account"],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        }"#).must();

        let describe: SObjectDescribe = serde_json::from_value(describe_json).must();
        let dbml = generate_dbml(&describe);

        assert!(dbml.contains("Table Account {"));
        assert!(dbml.contains("Id varchar(18) [pk, note: 'Account ID']"));
        assert!(dbml.contains("Name varchar [not null, note: 'Account Name']"));
        assert!(dbml.contains("ParentId varchar(18) [note: 'Parent Account ID']"));
        assert!(dbml.contains("Ref: Account.ParentId > Account.Id"));
    }
}
