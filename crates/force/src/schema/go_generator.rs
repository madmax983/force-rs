//! Go struct generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use std::fmt::Write;

/// Generates a Go struct definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_go_struct(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_go_struct(&mut out, describe);
    out
}

/// Writes a Go struct definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_go_struct(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(
        out,
        "// {} represents the Salesforce {} object.",
        describe.name, describe.name
    );
    let _ = writeln!(out, "type {} struct {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let go_type = map_type(&field.type_);
        let field_name = if field.name.is_empty() {
            "Unknown".to_string()
        } else {
            let mut c = field.name.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        };
        let json_tag = format!("`json:\"{}\"`", field.name);
        let ptr = "";
        let _ = writeln!(out, "    {} {}{} {}", field_name, ptr, go_type, json_tag);
    }
    out.push_str("}\n");
}

#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "float64",
        FieldType::Date => "time.Time",
        FieldType::Base64 => "[]byte",
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_go_struct() {
        let mut describe = MockSObjectDescribeBuilder::new("Account").build();
        describe.fields = vec![
            MockFieldDescribeBuilder::new("Id", FieldType::Id).build(),
            MockFieldDescribeBuilder::new("Name", FieldType::String).build(),
            MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency).build(),
        ];

        let out = generate_go_struct(&describe);
        assert!(out.contains("type Account struct {"));
        assert!(out.contains("Id string `json:\"Id\"`"));
        assert!(out.contains("Name string `json:\"Name\"`"));
        assert!(out.contains("AnnualRevenue float64 `json:\"AnnualRevenue\"`"));
    }
}
