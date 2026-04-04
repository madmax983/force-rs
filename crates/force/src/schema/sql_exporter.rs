//! SQL DDL Exporter.
//!
//! This module provides a utility to generate SQL Data Definition Language (DDL)
//! statements from Salesforce SObjectDescribe metadata. It maps Salesforce field types
//! to standard SQL column types, making it easy to create local databases (like SQLite
//! or PostgreSQL) that mirror Salesforce objects.
//!
//! # Example
//!
//! ```no_run
//! # use force::api::rest_operation::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::generate_ddl;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let describe = client.rest().describe("Account").await?;
//!
//! let ddl = generate_ddl(&describe);
//! println!("{}", ddl);
//! // CREATE TABLE Account (
//! //     Id VARCHAR(18) PRIMARY KEY,
//! //     Name VARCHAR(255),
//! //     ...
//! // );
//! # Ok(())
//! # }
//! ```

use crate::api::rest::describe::{FieldType, SObjectDescribe};

/// Generates a `CREATE TABLE` SQL statement for the given SObject describe metadata.
///
/// The generated SQL uses standard types that are generally compatible with
/// PostgreSQL and SQLite. The primary key is automatically set to the `Id` field
/// if it exists.
#[must_use]
pub fn generate_ddl(describe: &SObjectDescribe) -> String {
    let mut ddl = String::with_capacity(1024);
    write_ddl(&mut ddl, describe);
    ddl
}

/// Writes a `CREATE TABLE` SQL statement for the given SObject describe metadata into the provided buffer.
pub fn write_ddl(ddl: &mut String, describe: &SObjectDescribe) {
    use std::fmt::Write;

    let _ = writeln!(ddl, "CREATE TABLE {} (", describe.name);

    // Sort fields alphabetically to ensure deterministic output,
    // but always put 'Id' first if it exists.
    // ⚡ Bolt: Collecting references to fields instead of deep cloning the entire `describe.fields` Vec avoids significant heap allocation per field.
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| {
        if a.name == "Id" {
            std::cmp::Ordering::Less
        } else if b.name == "Id" {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    // ⚡ Bolt: Append directly to `ddl` buffer instead of collecting into an intermediate `field_defs` Vec and calling `.join(",\n")`.
    let mut first = true;
    for field in fields {
        if !first {
            ddl.push_str(",\n");
        }
        first = false;

        let _ = write!(ddl, "    {} ", field.name);
        write_field_type(ddl, &field.type_, field.length);

        if field.name == "Id" {
            ddl.push_str(" PRIMARY KEY");
        } else if !field.nillable && !field.defaulted_on_create {
            // If it's required and doesn't have a default on create, it should probably be NOT NULL
            // However, standard Salesforce behavior often allows inserts when a trigger sets the value,
            // but for SQL consistency we can add NOT NULL for strict mappings.
            ddl.push_str(" NOT NULL");
        }
    }

    ddl.push_str("\n);");
}

/// Maps a Salesforce `FieldType` to a standard SQL data type.
fn write_field_type(out: &mut String, field_type: &FieldType, length: i32) {
    use std::fmt::Write;
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
    use crate::api::rest::describe::FieldDescribe;

    fn mock_field(
        name: &str,
        type_: FieldType,
        length: i32,
        nillable: bool,
        defaulted: bool,
    ) -> FieldDescribe {
        FieldDescribe {
            aggregatable: true,
            auto_number: false,
            byte_length: length,
            calculated: false,
            calculated_formula: None,
            cascade_delete: false,
            case_sensitive: false,
            compound_field_name: None,
            controller_name: None,
            createable: true,
            custom: false,
            default_value: None,
            default_value_formula: None,
            defaulted_on_create: defaulted,
            dependent_picklist: false,
            deprecated_and_hidden: false,
            digits: 0,
            display_location_in_decimal: false,
            encrypted: false,
            external_id: false,
            extra_type_info: None,
            filterable: true,
            filtered_lookup_info: None,
            formula_treat_blanks_as: None,
            groupable: true,
            high_scale_number: false,
            html_formatted: false,
            id_lookup: name == "Id",
            inline_help_text: None,
            label: name.to_string(),
            length,
            mask: None,
            mask_type: None,
            name: name.to_string(),
            name_field: name == "Name",
            name_pointing: false,
            nillable,
            permissionable: false,
            picklist_values: None,
            polymorphic_foreign_key: false,
            precision: 0,
            query_by_distance: false,
            reference_target_field: None,
            reference_to: vec![],
            relationship_name: None,
            relationship_order: None,
            restricted_delete: false,
            restricted_picklist: false,
            scale: 0,
            search_prefixes_supported: None,
            soap_type: "xsd:string".to_string(),
            sortable: true,
            type_,
            unique: false,
            updateable: true,
            write_requires_master_read: false,
        }
    }

    #[test]
    fn test_generate_ddl() {
        let describe = SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: Some("001".to_string()),
            label: "Account".to_string(),
            label_plural: "Accounts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Account".to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            urls: std::collections::HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
            fields: vec![
                mock_field("Id", FieldType::Id, 18, false, true),
                mock_field("Name", FieldType::String, 255, false, false),
                mock_field("NumberOfEmployees", FieldType::Int, 0, true, false),
                mock_field("AnnualRevenue", FieldType::Currency, 0, true, false),
                mock_field("IsActive", FieldType::Boolean, 0, false, true),
            ],
        };

        let ddl = generate_ddl(&describe);

        let expected = "CREATE TABLE Account (\n    Id VARCHAR(18) PRIMARY KEY,\n    AnnualRevenue DOUBLE PRECISION,\n    IsActive BOOLEAN,\n    Name VARCHAR(255) NOT NULL,\n    NumberOfEmployees INTEGER\n);";
        assert_eq!(ddl, expected);
    }
}
