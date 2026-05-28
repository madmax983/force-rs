//! Elasticsearch Mapping Generator for Salesforce SObject Describe metadata.
//!
//! Generates Elasticsearch/OpenSearch index mappings based on Salesforce object schemas.
//!
//! # The Spark
//! We have `SObjectDescribe` which contains rich field metadata.
//! We often want to sync Salesforce data to Elasticsearch for full-text search.
//! Why write mappings by hand when we can generate them directly from the schema?

#[cfg(feature = "schema")]
use crate::types::describe::{FieldDescribe, FieldType, SObjectDescribe};
#[cfg(feature = "schema")]
use serde_json::{Value, json};

/// Generates an Elasticsearch index mapping for a given SObject.
#[cfg(feature = "schema")]
pub fn generate_elasticsearch_mapping(describe: &SObjectDescribe) -> Value {
    let mut properties = serde_json::Map::new();

    for field in &describe.fields {
        properties.insert(field.name.clone(), generate_field_mapping(field));
    }

    json!({
        "mappings": {
            "_source": {
                "enabled": true
            },
            "properties": properties
        }
    })
}

#[cfg(feature = "schema")]
fn generate_field_mapping(field: &FieldDescribe) -> Value {
    match field.type_ {
        FieldType::String | FieldType::Email | FieldType::Phone | FieldType::Url => {
            json!({
                "type": "text",
                "fields": {
                    "keyword": {
                        "type": "keyword",
                        "ignore_above": 256
                    }
                }
            })
        }
        FieldType::Textarea => {
            json!({ "type": "text" })
        }
        FieldType::Boolean => {
            json!({ "type": "boolean" })
        }
        FieldType::Int => {
            json!({ "type": "integer" })
        }
        FieldType::Double | FieldType::Currency | FieldType::Percent => {
            json!({ "type": "double" })
        }
        FieldType::Date => {
            json!({
                "type": "date",
                "format": "yyyy-MM-dd"
            })
        }
        FieldType::Datetime => {
            json!({
                "type": "date",
                "format": "strict_date_optional_time||epoch_millis"
            })
        }
        FieldType::Location => {
            json!({ "type": "geo_point" })
        }
        FieldType::Base64 => {
            json!({ "type": "binary" })
        }
        _ => {
            json!({ "type": "keyword" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_describe::MockFieldDescribeBuilder;

    #[test]
    fn test_generate_elasticsearch_mapping() {
        let describe = SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: Some("001".to_string()),
            label: "Account".to_string(),
            label_plural: "Accounts".to_string(),
            layoutable: true,
            mergeable: true,
            mru_enabled: true,
            name: "Account".to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            urls: std::collections::HashMap::new(),
            child_relationships: vec![],
            record_type_infos: vec![],
            fields: vec![
                MockFieldDescribeBuilder::new("Id", FieldType::Id).build(),
                MockFieldDescribeBuilder::new("Name", FieldType::String).build(),
                MockFieldDescribeBuilder::new("AnnualRevenue", FieldType::Currency).build(),
                MockFieldDescribeBuilder::new("CreatedDate", FieldType::Datetime).build(),
                MockFieldDescribeBuilder::new("IsActive", FieldType::Boolean).build(),
                MockFieldDescribeBuilder::new("BillingLocation", FieldType::Location).build(),
            ],
        };

        let mapping = generate_elasticsearch_mapping(&describe);

        let props = &mapping["mappings"]["properties"];
        assert_eq!(props["Id"]["type"], "keyword");
        assert_eq!(props["Name"]["type"], "text");
        assert_eq!(props["Name"]["fields"]["keyword"]["type"], "keyword");
        assert_eq!(props["AnnualRevenue"]["type"], "double");
        assert_eq!(props["CreatedDate"]["type"], "date");
        assert_eq!(props["IsActive"]["type"], "boolean");
        assert_eq!(props["BillingLocation"]["type"], "geo_point");
    }
}