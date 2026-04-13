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
pub fn write_rust_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/// {}", describe.label);
    out.push_str("#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]\n");
    out.push_str("pub struct ");
    write_pascal_case(out, &describe.name);
    out.push_str(" {\n");

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

    for field in fields {
        let _ = writeln!(out, "    /// {}", field.label);
        let _ = writeln!(out, "    #[serde(rename = \"{}\")]", field.name);
        let rust_type = map_type(&field.type_);
        // ⚡ Bolt: Write type directly to buffer instead of allocating intermediate String via format! or to_string()
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
    fn test_generate_struct() {
        use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .label("Account ID")
                    .length(18)
                    .byte_length(18)
                    .nillable(false)
                    .createable(false)
                    .updateable(false)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .label("Account Name")
                    .length(255)
                    .byte_length(255)
                    .nillable(true)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .label("Active")
                    .length(0)
                    .byte_length(0)
                    .nillable(false)
                    .createable(true)
                    .updateable(true)
                    .permissionable(false)
                    .defaulted_on_create(true)
                    .build(),
            )
            .build();

        let mut describe = describe;
        describe.label = "Account Object".to_string(); // override default label from the builder

        let result = generate_rust_struct(&describe);

        let expected = r#"/// Account Object
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Account {
    /// Account ID
    #[serde(rename = "Id")]
    pub id: String,
    /// Active
    #[serde(rename = "IsActive")]
    pub is_active: bool,
    /// Account Name
    #[serde(rename = "Name")]
    pub name: Option<String>,
}
"#;

        assert_eq!(result, expected);
    }
}
