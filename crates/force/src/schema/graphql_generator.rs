//! GraphQL Schema Generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a GraphQL type definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_graphql_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 64);
    write_graphql_schema(&mut out, describe);
    out
}

/// Writes a GraphQL type definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_graphql_schema(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "type {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let gql_type = map_type(&field.type_);
        let nullability_modifier = if field.nillable { "" } else { "!" };
        let _ = writeln!(
            out,
            "  {}: {}{}",
            field.name, gql_type, nullability_modifier
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a GraphQL type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Id | FieldType::Reference => "ID",
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Float",
        // Typically serialized as strings
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_graphql_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .nillable(false)
                    .updateable(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .nillable(false)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .nillable(true)
                    .updateable(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean)
                    .nillable(true)
                    .updateable(true)
                    .build(),
            )
            .build();

        let graphql_code = generate_graphql_schema(&describe);

        let expected = "type Account {\n  Id: ID!\n  IsActive: Boolean\n  Name: String!\n  NumberOfEmployees: Int\n}\n";
        assert_eq!(graphql_code, expected);
    }
}
