//! SQL Migration Generator.
//!
//! This module provides a utility to generate SQL migrations (`ALTER TABLE` statements)
//! by comparing two `SObjectDescribe` payloads.

use crate::schema::schema_diff::SchemaDiffResult;
use crate::types::describe::FieldType;
use std::fmt::Write;

/// Generates SQL migration statements by comparing an old and new schema describe.
///
/// Uses `SchemaDiffResult` to identify added, removed, and changed fields.
///
/// # Arguments
///
/// * `table_name` - The name of the table to generate the migration for.
/// * `diff` - The `SchemaDiffResult` from comparing the old and new schemas.
#[must_use]
pub fn generate_sql_migration(table_name: &str, diff: &SchemaDiffResult<'_>) -> String {
    let mut sql = String::with_capacity(1024);

    if diff.is_empty() {
        return sql;
    }

    // Generate ALTER TABLE ... ADD COLUMN
    for field in &diff.added {
        let _ = write!(sql, "ALTER TABLE {} ADD COLUMN {} ", table_name, field.name);
        write_field_type(&mut sql, &field.type_, field.length);
        if !field.nillable && !field.defaulted_on_create && field.name != "Id" {
            sql.push_str(" NOT NULL");
        }
        sql.push_str(";\n");
    }

    // Generate ALTER TABLE ... DROP COLUMN
    for field in &diff.removed {
        let _ = writeln!(
            sql,
            "ALTER TABLE {} DROP COLUMN {};",
            table_name, field.name
        );
    }

    // Generate ALTER TABLE ... ALTER COLUMN ... TYPE
    for change in &diff.changed {
        // We cannot reliably determine length from FieldChange, so we assume default lengths
        // in write_field_type if length is not explicitly passed. For ALTER TYPE, we default to 255 for strings.
        let _ = write!(
            sql,
            "ALTER TABLE {} ALTER COLUMN {} TYPE ",
            table_name, change.name
        );
        write_field_type(&mut sql, change.new_type, 255);
        sql.push_str(";\n");
    }

    sql
}

/// Maps a Salesforce `FieldType` to a standard SQL data type.
/// This replicates the mapping logic from `sql_exporter`.
fn write_field_type(out: &mut String, field_type: &FieldType, length: i32) {
    match field_type {
        FieldType::Id | FieldType::Reference => out.push_str("VARCHAR(18)"),
        FieldType::String
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url
        | FieldType::Picklist
        | FieldType::Multipicklist
        | FieldType::Combobox => {
            if length > 0 {
                let _ = write!(out, "VARCHAR({})", length);
            } else {
                out.push_str("VARCHAR(255)");
            }
        }
        FieldType::Boolean => out.push_str("BOOLEAN"),
        FieldType::Int => out.push_str("INTEGER"),
        FieldType::Double | FieldType::Currency | FieldType::Percent => {
            out.push_str("DOUBLE PRECISION");
        }
        FieldType::Date => out.push_str("DATE"),
        FieldType::Datetime => out.push_str("TIMESTAMP"),
        FieldType::Time => out.push_str("TIME"),
        _ => out.push_str("TEXT"), // Fallback for complex/unknown types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::schema_diff::compare_schemas;
    use crate::test_utils::must::Must;
    use crate::types::describe::SObjectDescribe;
    use serde_json::json;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(
        name: &str,
        field_type: &str,
        length: i32,
        nillable: bool,
        defaulted: bool,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": [],
            "aggregatable": true, "autoNumber": false, "byteLength": length, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": defaulted, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": name == "Id", "length": length, "nameField": false, "namePointing": false, "nillable": nillable,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_sql_migration() {
        let old_schema = create_mock_describe(&json!([
            mock_field("Id", "id", 18, false, true),
            mock_field("Name", "string", 255, false, false),
            mock_field("OldField", "string", 255, true, false)
        ]));

        let new_schema = create_mock_describe(&json!([
            mock_field("Id", "id", 18, false, true),
            mock_field("Name", "textarea", 0, false, false), // Changed type
            mock_field("NewField", "int", 0, true, false)    // Added field
        ]));

        let diff = compare_schemas(&old_schema, &new_schema);
        let sql = generate_sql_migration("Account", &diff);

        let expected = "ALTER TABLE Account ADD COLUMN NewField INTEGER;\nALTER TABLE Account DROP COLUMN OldField;\nALTER TABLE Account ALTER COLUMN Name TYPE TEXT;\n";
        assert_eq!(sql, expected);
    }
}
