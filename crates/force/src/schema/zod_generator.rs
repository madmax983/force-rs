//! Zod schema generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Zod schema definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_zod_schema(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_zod_schema(&mut out, describe);
    out
}

/// Writes a Zod schema definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_zod_schema(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("import { z } from \"zod\";\n\n");
    out.push_str("/**\n");
    let _ = writeln!(out, " * {}", describe.label);
    out.push_str(" */\n");
    let _ = writeln!(out, "export const {}Schema = z.object({{", describe.name);

    // Sort fields alphabetically, Id first
    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let ts_type = map_type(&field.type_);
        let optional = if field.nillable { ".optional()" } else { "" };
        let _ = writeln!(out, "  {}: z.{}(){},", field.name, ts_type, optional);
    }

    out.push_str("});\n\n");
    let _ = writeln!(
        out,
        "export type {} = z.infer<typeof {}Schema>;",
        describe.name, describe.name
    );
}

/// Maps a Salesforce `FieldType` to a Zod primitive.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "boolean",
        FieldType::Int | FieldType::Double | FieldType::Currency | FieldType::Percent => "number",
        // Typically serialized as ISO 8601 strings
        _ => "string",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_zod_generator() {
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

        let zod_code = generate_zod_schema(&describe);

        let expected = r#"import { z } from "zod";

/**
 * Account
 */
export const AccountSchema = z.object({
  Id: z.string(),
  IsActive: z.boolean().optional(),
  Name: z.string(),
  NumberOfEmployees: z.number().optional(),
});

export type Account = z.infer<typeof AccountSchema>;
"#;
        assert_eq!(zod_code, expected);
    }
}
