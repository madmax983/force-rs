//! Schema Limits Analyzer for Salesforce SObject Describe metadata.
//!
//! This module provides a utility to analyze `SObjectDescribe` payloads against
//! common Salesforce org limits (e.g., maximum custom fields, relationship limits),
//! helping developers proactively identify schema bloat before hitting hard platform limits.
//!
//! # Example
//!
//! ```no_run
//! # use force::api::RestOperation;
//! # use force::client::ForceClientBuilder;
//! # use force::schema::analyze_schema_limits;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let describe = client.rest().describe("Account").await?;
//!
//! let limits = analyze_schema_limits(&describe);
//!
//! println!("Custom Fields: {}/500", limits.custom_fields);
//! println!("Relationship Fields: {}/40", limits.relationship_fields);
//! println!("Is Approaching Custom Field Limit? {}", limits.approaching_custom_field_limit);
//! # Ok(())
//! # }
//! ```

use crate::types::describe::{FieldType, SObjectDescribe};

/// Configurable thresholds for limits analysis
const CUSTOM_FIELD_LIMIT: usize = 500; // Common enterprise limit
const RELATIONSHIP_FIELD_LIMIT: usize = 40; // Max lookup/master-detail per object
const WARNING_THRESHOLD_PERCENT: f64 = 0.8; // Warn at 80% capacity

/// Insights regarding how close an SObject is to Salesforce platform limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaLimitInsights {
    /// Number of custom fields present.
    pub custom_fields: usize,
    /// Number of relationship (reference) fields present.
    pub relationship_fields: usize,
    /// Number of roll-up summary fields present (usually max 25 or 40 depending on org type, though hard to determine precisely without full metadata API).
    pub rollup_summary_fields: usize,

    /// Are custom fields >= 80% of limit?
    pub approaching_custom_field_limit: bool,
    /// Are relationship fields >= 80% of limit?
    pub approaching_relationship_limit: bool,
}

/// Analyzes an `SObjectDescribe` against common Salesforce limits.
#[must_use]
pub fn analyze_schema_limits(describe: &SObjectDescribe) -> SchemaLimitInsights {
    let mut custom_fields = 0;
    let mut relationship_fields = 0;

    for field in &describe.fields {
        if field.custom {
            custom_fields += 1;
        }

        if matches!(field.type_, FieldType::Reference) {
            relationship_fields += 1;
        }
    }

    let approaching_custom_field_limit =
        (custom_fields as f64) >= (CUSTOM_FIELD_LIMIT as f64 * WARNING_THRESHOLD_PERCENT);

    let approaching_relationship_limit = (relationship_fields as f64)
        >= (RELATIONSHIP_FIELD_LIMIT as f64 * WARNING_THRESHOLD_PERCENT);

    SchemaLimitInsights {
        custom_fields,
        relationship_fields,
        rollup_summary_fields: 0, // Placeholder, describe doesn't reliably expose rollups natively without deeper metadata API checks
        approaching_custom_field_limit,
        approaching_relationship_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;
    use serde_json::json;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json
        });
        serde_json::from_value(describe_json).must()
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn mock_field(name: &str, field_type: &str, custom: bool) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "referenceTo": if field_type == "reference" { vec!["Account"] } else { vec![] },
            "custom": custom,
            "nillable": true,
            "defaultedOnCreate": false,
            "calculated": false,
            "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_analyze_schema_limits_safe() {
        let describe = create_mock_describe(&json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", false),
            mock_field("Custom1__c", "string", true),
            mock_field("Rel1__c", "reference", true)
        ]));

        let limits = analyze_schema_limits(&describe);

        assert_eq!(limits.custom_fields, 2);
        assert_eq!(limits.relationship_fields, 1);
        assert!(!limits.approaching_custom_field_limit);
        assert!(!limits.approaching_relationship_limit);
    }

    #[test]
    fn test_analyze_schema_limits_approaching_custom_limit() {
        let mut fields = Vec::new();
        // Add 400 custom fields (400 >= 500 * 0.8 is 400 >= 400)
        for i in 0..400 {
            fields.push(mock_field(&format!("Custom{}__c", i), "string", true));
        }

        let describe = create_mock_describe(&serde_json::Value::Array(fields));
        let limits = analyze_schema_limits(&describe);

        assert_eq!(limits.custom_fields, 400);
        assert!(limits.approaching_custom_field_limit);
        assert!(!limits.approaching_relationship_limit);
    }

    #[test]
    fn test_analyze_schema_limits_approaching_relationship_limit() {
        let mut fields = Vec::new();
        // Add 32 relationship fields (32 >= 40 * 0.8 is 32 >= 32)
        for i in 0..32 {
            fields.push(mock_field(&format!("Rel{}__c", i), "reference", true));
        }

        let describe = create_mock_describe(&serde_json::Value::Array(fields));
        let limits = analyze_schema_limits(&describe);

        assert_eq!(limits.relationship_fields, 32);
        assert!(limits.approaching_relationship_limit);
        assert!(!limits.approaching_custom_field_limit);
    }
}
