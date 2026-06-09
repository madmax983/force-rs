//! Prisma schema generator for Salesforce SObject Describe metadata.
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Prisma schema model from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_prisma_model(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_prisma_model(&mut out, describe);
    out
}

/// Writes a Prisma schema model from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_prisma_model(out: &mut String, describe: &SObjectDescribe) {
    let _ = writeln!(out, "/// {}", describe.label);
    let _ = writeln!(out, "model {} {{", describe.name);

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let prisma_type = map_type(&field.type_);
        let optional = if field.nillable { "?" } else { "" };

        let mut attributes = String::new();
        if field.name == "Id" {
            attributes.push_str(" @id");
        }
        if field.unique {
            attributes.push_str(" @unique");
        }

        let _ = writeln!(
            out,
            "  {} {}{}{}",
            field.name, prisma_type, optional, attributes
        );
    }

    out.push_str("}\n");
}

/// Maps a Salesforce `FieldType` to a Prisma type.
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "Boolean",
        FieldType::Int => "Int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "Float",
        FieldType::Date | FieldType::Datetime => "DateTime",
        // Most Salesforce types translate cleanly to String in Prisma
        _ => "String",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_prisma_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(
                MockFieldDescribeBuilder::new("Id", FieldType::Id)
                    .nillable(false)
                    .unique(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("Name", FieldType::String)
                    .nillable(false)
                    .unique(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int)
                    .nillable(true)
                    .unique(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency)
                    .nillable(true)
                    .unique(false)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("ExternalId__c", FieldType::String)
                    .nillable(true)
                    .unique(true)
                    .build(),
            )
            .field(
                MockFieldDescribeBuilder::new("CreatedDate", FieldType::Datetime)
                    .nillable(false)
                    .unique(false)
                    .build(),
            )
            .build();

        let prisma = generate_prisma_model(&describe);

        let expected = "/// Account\nmodel Account {\n  Id String @id\n  AnnualRevenue Float?\n  CreatedDate DateTime\n  ExternalId__c String? @unique\n  Name String\n  NumberOfEmployees Int?\n}\n";
        assert_eq!(prisma, expected);
    }
}
