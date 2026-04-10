#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Preview utility to generate Rust structs from SObject describe metadata.
#[cfg(feature = "schema")]
/// Generates a Rust struct definition from an SObject describe result.
///
/// This will generate a struct with `serde` rename attributes to match the
/// Salesforce API field names, and map the Salesforce types to appropriate
/// Rust types.
pub fn generate_rust_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_rust_struct(&mut out, describe);
    out
}

/// Writes a Rust struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_rust_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/// {}", describe.label);
    out.push_str("#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]\n");
    out.push_str("pub struct ");
    write_pascal_case(out, &describe.name);
    out.push_str(" {\n");

    for field in &describe.fields {
        let _ = writeln!(out, "    /// {}", field.label);
        let _ = writeln!(out, "    #[serde(rename = \"{}\")]", field.name);
        let rust_type = map_type(&field.type_);
        out.push_str("    pub ");
        write_snake_case(out, &field.name);
        out.push_str(": ");
        if field.nillable {
            let _ = writeln!(out, "Option<{}>,", rust_type);
        } else {
            let _ = writeln!(out, "{},", rust_type);
        }
    }

    out.push_str("}\n");
}

/// Converts a string to PascalCase.
///
/// ⚡ Bolt: Uses `String::with_capacity` to prevent reallocation.
fn pascal_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    write_pascal_case(&mut result, s);
    result
}

/// Writes a string as PascalCase directly to a buffer.
fn write_pascal_case(out: &mut String, s: &str) {
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            out.push(c);
        }
    }
}

/// Converts a string to snake_case.
///
/// ⚡ Bolt: Uses `String::with_capacity` to prevent reallocation and iterates
/// over `chars()` to remove intermediate `Vec<char>` allocation.
fn snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    write_snake_case(&mut result, s);
    result
}

/// Writes a string as snake_case directly to a buffer.
fn write_snake_case(out: &mut String, s: &str) {
    let start_len = out.len();
    let mut prev_char: Option<char> = None;

    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if let Some(p) = prev_char {
                if !p.is_ascii_uppercase() && p != '_' {
                    out.push('_');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
        prev_char = Some(c);
    }

    if &out[start_len..] == "type" {
        out.push('_');
    }
}

fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "i64",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "f64",
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert_eq!(snake_case("Account"), "account");
        assert_eq!(snake_case("AccountId"), "account_id");
        assert_eq!(snake_case("IsActive"), "is_active");
        assert_eq!(snake_case("type"), "type_");
        assert_eq!(snake_case("ID"), "id");
        assert_eq!(snake_case("camelCase"), "camel_case");
        assert_eq!(snake_case("Custom_Field__c"), "custom_field__c");
        assert_eq!(snake_case("URL"), "url");
        assert_eq!(snake_case("someURLField"), "some_urlfield");
        assert_eq!(snake_case("Already_Snake_Case"), "already_snake_case");
    }

    #[test]
    fn test_map_type() {
        assert_eq!(map_type(&FieldType::Boolean), "bool");
        assert_eq!(map_type(&FieldType::Int), "i64");
        assert_eq!(map_type(&FieldType::Double), "f64");
        assert_eq!(map_type(&FieldType::Currency), "f64");
        assert_eq!(map_type(&FieldType::Percent), "f64");
        assert_eq!(map_type(&FieldType::String), "String");
        assert_eq!(map_type(&FieldType::Picklist), "String");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(pascal_case("account"), "Account");
        assert_eq!(pascal_case("account_id"), "AccountId");
        assert_eq!(pascal_case("ID"), "ID");
        assert_eq!(pascal_case("custom_field__c"), "CustomFieldC");
        assert_eq!(pascal_case("camelCase"), "CamelCase");
        assert_eq!(pascal_case("AlreadyPascalCase"), "AlreadyPascalCase");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_generate_struct() {
        use crate::test_support::Must;
        // Construct JSON since SObjectDescribe has many required fields
        let json = r#"{
            "activateable": false,
            "createable": true,
            "custom": false,
            "customSetting": false,
            "deletable": true,
            "deprecatedAndHidden": false,
            "feedEnabled": true,
            "hasSubtypes": false,
            "isSubtype": false,
            "keyPrefix": "001",
            "label": "Account Object",
            "labelPlural": "Accounts",
            "layoutable": true,
            "mergeable": true,
            "mruEnabled": true,
            "name": "Account",
            "queryable": true,
            "replicateable": true,
            "retrieveable": true,
            "searchable": true,
            "triggerable": true,
            "undeletable": true,
            "updateable": true,
            "urls": {
                "sobject": "/services/data/v60.0/sobjects/Account"
            },
            "fields": [
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 18,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": false,
                    "custom": false,
                    "defaultedOnCreate": true,
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
                    "idLookup": true,
                    "label": "Account ID",
                    "length": 18,
                    "name": "Id",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "tns:ID",
                    "sortable": true,
                    "type": "id",
                    "unique": false,
                    "updateable": false,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 255,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": true,
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
                    "idLookup": false,
                    "label": "Account Name",
                    "length": 255,
                    "name": "Name",
                    "nameField": true,
                    "namePointing": false,
                    "nillable": true,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:string",
                    "sortable": true,
                    "type": "string",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                },
                {
                    "aggregatable": true,
                    "autoNumber": false,
                    "byteLength": 0,
                    "calculated": false,
                    "cascadeDelete": false,
                    "caseSensitive": false,
                    "createable": true,
                    "custom": false,
                    "defaultedOnCreate": true,
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
                    "idLookup": false,
                    "label": "Active",
                    "length": 0,
                    "name": "IsActive",
                    "nameField": false,
                    "namePointing": false,
                    "nillable": false,
                    "permissionable": false,
                    "polymorphicForeignKey": false,
                    "precision": 0,
                    "queryByDistance": false,
                    "referenceTo": [],
                    "restrictedDelete": false,
                    "restrictedPicklist": false,
                    "scale": 0,
                    "soapType": "xsd:boolean",
                    "sortable": true,
                    "type": "boolean",
                    "unique": false,
                    "updateable": true,
                    "writeRequiresMasterRead": false
                }
            ],
            "childRelationships": [],
            "recordTypeInfos": []
        }"#;

        let describe: SObjectDescribe = serde_json::from_str(json).must();

        let result = generate_rust_struct(&describe);

        let expected = r#"/// Account Object
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Account {
    /// Account ID
    #[serde(rename = "Id")]
    pub id: String,
    /// Account Name
    #[serde(rename = "Name")]
    pub name: Option<String>,
    /// Active
    #[serde(rename = "IsActive")]
    pub is_active: bool,
}
"#;

        assert_eq!(result, expected);
    }
}
