//! Pydantic model generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Generates a Python Pydantic model definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_pydantic_model(describe: &SObjectDescribe) -> String {
    let mut out = String::with_capacity(describe.fields.len() * 128);
    write_pydantic_model(&mut out, describe);
    out
}

/// Writes a Python Pydantic model definition from an SObject describe result directly to a string buffer.
#[cfg(feature = "schema")]
pub fn write_pydantic_model(out: &mut String, describe: &SObjectDescribe) {
    out.push_str("from typing import Optional\n");
    out.push_str("from pydantic import BaseModel, Field\n\n");

    let _ = writeln!(out, "class {}(BaseModel):", describe.name);
    out.push_str("    \"\"\"\n");
    let _ = writeln!(out, "    {}", describe.label);
    out.push_str("    \"\"\"\n");

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        let py_type = map_type(&field.type_);
        if field.nillable {
            let _ = writeln!(
                out,
                "    {}: Optional[{}] = Field(None, description=\"{}\")",
                field.name, py_type, field.name
            );
        } else {
            let _ = writeln!(
                out,
                "    {}: {} = Field(..., description=\"{}\")",
                field.name, py_type, field.name
            );
        }
    }
}

/// Maps a Salesforce `FieldType` to a Python type.
#[cfg(feature = "schema")]
fn map_type(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Boolean => "bool",
        FieldType::Int => "int",
        FieldType::Double | FieldType::Currency | FieldType::Percent => "float",
        // Typically serialized as strings
        _ => "str",
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_support::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_pydantic_generator() {
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

        let pydantic_code = generate_pydantic_model(&describe);

        let expected = r#"from typing import Optional
from pydantic import BaseModel, Field

class Account(BaseModel):
    """
    Account
    """
    Id: str = Field(..., description="Id")
    IsActive: Optional[bool] = Field(None, description="IsActive")
    Name: str = Field(..., description="Name")
    NumberOfEmployees: Optional[int] = Field(None, description="NumberOfEmployees")
"#;
        assert_eq!(pydantic_code, expected);
    }
}
