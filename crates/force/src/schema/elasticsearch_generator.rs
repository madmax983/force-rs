//! Elasticsearch mapping generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates an Elasticsearch mapping definition from an SObject describe result.
#[cfg(feature = "schema")]
pub fn generate_elasticsearch_mapping(describe: &SObjectDescribe) -> Value {
    let mut properties = serde_json::Map::new();

    let mut fields: Vec<&_> = describe.fields.iter().collect();
    fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

    for field in fields {
        properties.insert(field.name.clone(), map_type(&field.type_));
    }

    json!({
        "mappings": {
            "properties": properties
        }
    })
}

#[cfg(feature = "schema")]
fn map_type(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::Id
        | FieldType::Reference
        | FieldType::Picklist
        | FieldType::Combobox
        | FieldType::Email
        | FieldType::Phone
        | FieldType::Url => {
            json!({ "type": "keyword" })
        }
        FieldType::String | FieldType::Textarea => {
            json!({ "type": "text" })
        }
        FieldType::Int => {
            json!({ "type": "integer" })
        }
        FieldType::Double | FieldType::Currency | FieldType::Percent => {
            json!({ "type": "double" })
        }
        FieldType::Boolean => {
            json!({ "type": "boolean" })
        }
        FieldType::Date | FieldType::Datetime => {
            json!({ "type": "date" })
        }
        _ => json!({ "type": "text" }),
    }
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};

    #[test]
    fn test_elasticsearch_mapping_generator() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(MockFieldDescribeBuilder::new("Id", FieldType::Id).build())
            .field(MockFieldDescribeBuilder::new("Name", FieldType::String).build())
            .field(MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int).build())
            .field(MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency).build())
            .field(MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean).build())
            .field(MockFieldDescribeBuilder::new("CreatedDate", FieldType::Datetime).build())
            .build();

        let mapping = generate_elasticsearch_mapping(&describe);

        assert_eq!(mapping["mappings"]["properties"]["Id"]["type"], "keyword");
        assert_eq!(mapping["mappings"]["properties"]["Name"]["type"], "text");
        assert_eq!(
            mapping["mappings"]["properties"]["NumberOfEmployees"]["type"],
            "integer"
        );
        assert_eq!(
            mapping["mappings"]["properties"]["AnnualRevenue"]["type"],
            "double"
        );
        assert_eq!(
            mapping["mappings"]["properties"]["IsActive"]["type"],
            "boolean"
        );
        assert_eq!(
            mapping["mappings"]["properties"]["CreatedDate"]["type"],
            "date"
        );
    }
}
