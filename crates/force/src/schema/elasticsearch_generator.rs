//! Elasticsearch / OpenSearch index mapping generator for Salesforce SObject Describe metadata.
//!
//! Generates JSON index mappings based on Salesforce object schemas, allowing easy
//! synchronization of CRM data into Elasticsearch for full-text search and analytics.

#[cfg(feature = "schema")]
use crate::types::describe::{FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Map, Value};

/// Generates an Elasticsearch / OpenSearch JSON mapping definition for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_elasticsearch_mapping(describe: &SObjectDescribe) -> Value {
    let mut properties = Map::new();
    let mut sorted_fields: Vec<&_> = describe.fields.iter().collect();
    sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));
    for field in sorted_fields {
        let es_type = match field.type_ {
            FieldType::Int => "integer",
            FieldType::Double | FieldType::Percent | FieldType::Currency => "double",
            FieldType::Boolean => "boolean",
            FieldType::Date | FieldType::Datetime => "date",
            FieldType::Base64 => "binary",
            // Strings and Picklists
            FieldType::Id
            | FieldType::Reference
            | FieldType::Email
            | FieldType::Phone
            | FieldType::Url => "keyword",
            _ => "text",
        };
        let mut field_mapping = Map::new();
        field_mapping.insert("type".to_string(), Value::String(es_type.to_string()));
        // Add a raw keyword sub-field for text types to support aggregations
        if es_type == "text" {
            let mut fields_obj = Map::new();
            let mut keyword_obj = Map::new();
            keyword_obj.insert("type".to_string(), Value::String("keyword".to_string()));
            keyword_obj.insert("ignore_above".to_string(), Value::Number(256.into()));
            fields_obj.insert("keyword".to_string(), Value::Object(keyword_obj));
            field_mapping.insert("fields".to_string(), Value::Object(fields_obj));
        }
        properties.insert(field.name.clone(), Value::Object(field_mapping));
    }
    let mut mapping = Map::new();
    mapping.insert("properties".to_string(), Value::Object(properties));
    let mut root = Map::new();
    root.insert("mappings".to_string(), Value::Object(mapping));
    Value::Object(root)
}

#[cfg(test)]
#[cfg(feature = "schema")]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::{MockFieldDescribeBuilder, MockSObjectDescribeBuilder};
    use crate::test_utils::must::MustMsg;
    #[test]
    fn test_generate_elasticsearch_mapping() {
        let describe = MockSObjectDescribeBuilder::new("Account")
            .field(MockFieldDescribeBuilder::new("Id", FieldType::Id).build())
            .field(MockFieldDescribeBuilder::new("Name", FieldType::String).build())
            .field(MockFieldDescribeBuilder::new("NumberOfEmployees", FieldType::Int).build())
            .field(MockFieldDescribeBuilder::new("CreatedDate", FieldType::Datetime).build())
            .build();
        let mapping = generate_elasticsearch_mapping(&describe);
        let mappings_obj = mapping
            .get("mappings")
            .must_msg("Missing mappings")
            .as_object()
            .must_msg("mappings not object");
        let properties = mappings_obj
            .get("properties")
            .must_msg("Missing properties")
            .as_object()
            .must_msg("properties not object");
        assert_eq!(
            properties.get("Id").must_msg("Missing Id")["type"],
            "keyword"
        );
        assert_eq!(
            properties.get("Name").must_msg("Missing Name")["type"],
            "text"
        );
        assert_eq!(
            properties.get("Name").must_msg("Missing Name")["fields"]["keyword"]["type"],
            "keyword"
        );
        assert_eq!(
            properties
                .get("NumberOfEmployees")
                .must_msg("Missing NumberOfEmployees")["type"],
            "integer"
        );
        assert_eq!(
            properties
                .get("CreatedDate")
                .must_msg("Missing CreatedDate")["type"],
            "date"
        );
    }
}
