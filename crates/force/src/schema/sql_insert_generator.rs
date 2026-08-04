//! SQL Insert Generator.
#[cfg(feature = "schema")]
use crate::types::describe::SObjectDescribe;
#[cfg(feature = "schema")]
use crate::schema::generate_mock_data;

/// Generates a batch SQL INSERT statement for the given SObject.
#[cfg(feature = "schema")]
pub fn generate_sql_inserts(describe: &SObjectDescribe, count: usize) -> String {
    if count == 0 {
        return String::new();
    }

    let mock_json = generate_mock_data(describe);
    let Some(obj) = mock_json.as_object() else {
        return String::new();
    };

    if obj.is_empty() {
        return String::new();
    }

    // Pre-allocate to prevent intermediate allocations, adhering to best practices
    let mut out = String::with_capacity(50 + count * obj.len() * 30);

    out.push_str("INSERT INTO ");
    out.push_str(&describe.name);
    out.push_str(" (");

    let mut first_col = true;
    for key in obj.keys() {
        if !first_col {
            out.push_str(", ");
        }
        out.push_str(key);
        first_col = false;
    }
    out.push_str(") VALUES\n");

    for i in 0..count {
        out.push_str("  (");
        let mut first_val = true;
        for val in obj.values() {
            if !first_val {
                out.push_str(", ");
            }
            if let Some(s) = val.as_str() {
                out.push('\'');
                out.push_str(s);
                out.push('\'');
            } else if let Some(n) = val.as_number() {
                out.push_str(&n.to_string());
            } else if let Some(b) = val.as_bool() {
                out.push_str(if b { "TRUE" } else { "FALSE" });
            } else {
                out.push_str("NULL");
            }
            first_val = false;
        }
        out.push(')');
        if i < count - 1 {
            out.push_str(",\n");
        } else {
            out.push_str(";\n");
        }
    }

    out
}

#[cfg(all(test, feature = "schema"))]
mod tests {
    use super::*;
    use crate::types::describe::FieldType;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_generate_sql_inserts_valid() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(MockFieldDescribeBuilder::new("Id", FieldType::Id).build())
            .field(MockFieldDescribeBuilder::new("Name", FieldType::String).build())
            .build();

        let sql = generate_sql_inserts(&describe, 2);
        assert!(sql.contains("INSERT INTO Account"));
        assert!(sql.contains("(Id, Name)") || sql.contains("(Name, Id)"));
        assert!(sql.contains("VALUES"));
        assert!(sql.contains("('001000000000000AAA', 'mock_string')") || sql.contains("('mock_string', '001000000000000AAA')"));
    }
}
