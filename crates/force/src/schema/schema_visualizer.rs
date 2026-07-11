use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::HashMap;
use std::fmt::Write;

use super::scanner::FieldUsageScanner;
use super::schema_analyzer::analyze_schema;
use super::schema_graph::SchemaGraph;

/// Generates a comprehensive Markdown report for the given SObject.
///
/// This combines `analyze_schema`, `SchemaGraph`, and `FieldUsageScanner`
/// into a single, unified visual report.
///
/// The report includes:
/// - Schema Insights (Complexity score, field counts, etc.)
/// - ER Diagram (Mermaid.js)
/// - Field Usage Statistics (Optional)
///
/// # Arguments
///
/// * `client` - The ForceClient to use.
/// * `sobject` - The API name of the SObject (e.g., "Account").
/// * `include_usage` - Whether to scan and include field population statistics.
pub async fn generate_visualizer_report<A: Authenticator>(
    client: &ForceClient<A>,
    sobject: &str,
    include_usage: bool,
) -> Result<String> {
    let describe = client.rest().describe(sobject).await?;

    let insights = analyze_schema(&describe);

    let mut md = String::with_capacity(2048);

    // ⚡ Bolt: Use `writeln!` directly to the `md` buffer instead of `format!` and `push_str`
    // to avoid intermediate String heap allocations for the report output.
    let _ = writeln!(md, "# Schema Report: {}\n", describe.label);
    let _ = writeln!(md, "**API Name:** `{}`", describe.name);
    let _ = writeln!(md, "**Custom:** {}\n", describe.custom);

    let _ = writeln!(md, "## Schema Insights\n");
    let _ = writeln!(
        md,
        "*   **Complexity Score:** {}",
        insights.complexity_score
    );
    let _ = writeln!(md, "*   **Total Fields:** {}", insights.total_fields);
    let _ = writeln!(
        md,
        "*   **Standard Fields:** {}",
        insights.standard_field_count
    );
    let _ = writeln!(md, "*   **Custom Fields:** {}", insights.custom_field_count);
    let _ = writeln!(
        md,
        "*   **Required Fields:** {}\n",
        insights.required_field_count
    );

    let mut graph = SchemaGraph::new(client);

    // ⚡ Bolt: We borrow fields from `describe` for usage stats before moving it into the graph.
    // However, to satisfy the borrow checker when sorting, we collect references to fields
    // rather than cloning the entire vector.
    let usages = if include_usage {
        let scanner = FieldUsageScanner::new(client);
        let u = scanner.scan(sobject).await?;
        let mut map = HashMap::with_capacity(u.len());
        for usage in u {
            map.insert(usage.name, usage.percentage);
        }

        let mut sorted_fields: Vec<&crate::types::describe::FieldDescribe> =
            describe.fields.iter().collect();
        sorted_fields.sort_by(|a, b| crate::schema::cmp_field_names(&a.name, &b.name));

        let mut usage_md = String::with_capacity(1024);
        let _ = writeln!(usage_md, "## Field Usage Statistics\n");
        let _ = writeln!(usage_md, "| Label | API Name | Populated % |");
        let _ = writeln!(usage_md, "|---|---|---|");

        for field in sorted_fields {
            if let Some(pct) = map.get(&field.name) {
                let _ = writeln!(
                    usage_md,
                    "| {} | `{}` | {:.1}% |",
                    field.label, field.name, pct
                );
            } else {
                let _ = writeln!(usage_md, "| {} | `{}` | N/A |", field.label, field.name);
            }
        }
        Some(usage_md)
    } else {
        None
    };

    graph.add_describe(describe);

    let _ = writeln!(md, "## Entity-Relationship Diagram\n");
    let _ = writeln!(md, "```mermaid");
    graph.write_mermaid(&mut md);
    let _ = writeln!(md, "```\n");

    if let Some(u_md) = usages {
        md.push_str(&u_md);
    }

    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }

    async fn create_test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        builder().authenticate(auth).build().await.must()
    }

    #[tokio::test]
    async fn test_schema_visualizer_generate_report() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
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
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Account ID", "createable": false,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": true, "referenceTo": [],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let md = generate_visualizer_report(&client, "Account", false)
            .await
            .must();

        assert!(md.contains("# Schema Report: Account"));
        assert!(md.contains("**API Name:** `Account`"));
        assert!(md.contains("## Schema Insights"));
        assert!(md.contains("## Entity-Relationship Diagram"));
        assert!(md.contains("```mermaid"));
        assert!(md.contains("erDiagram"));
        // Check that usage statistics section is absent since include_usage is false
        assert!(!md.contains("## Field Usage Statistics"));
    }
    #[tokio::test]
    async fn test_schema_visualizer_generate_report_with_usage() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let describe_json = serde_json::from_str::<serde_json::Value>(r#"{
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
            "fields": [
                {
                    "name": "Id", "type": "id", "label": "Account ID", "createable": false,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": true, "referenceTo": [],
                    "aggregatable": true, "byteLength": 18,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:id",
                    "sortable": true, "unique": true, "updateable": false, "writeRequiresMasterRead": false
                },
                {
                    "name": "Name", "type": "string", "label": "Account Name", "createable": true,
                    "autoNumber": false, "calculated": false, "custom": false, "nillable": false,
                    "defaultedOnCreate": false, "referenceTo": [],
                    "aggregatable": true, "byteLength": 255,
                    "cascadeDelete": false, "caseSensitive": false,
                    "dependentPicklist": false, "deprecatedAndHidden": false,
                    "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                    "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                    "idLookup": false, "length": 255, "nameField": true, "namePointing": false,
                    "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                    "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
                    "sortable": true, "unique": false, "updateable": true, "writeRequiresMasterRead": false
                }
            ]
        }"#).must();

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let soql_response = serde_json::from_str::<serde_json::Value>(
            r#"{
            "totalSize": 100,
            "done": true,
            "records": [
                {
                    "attributes": {"type": "AggregateResult"},
                    "expr0": 100,
                    "expr1": 50
                }
            ]
        }"#,
        )
        .must();

        Mock::given(method("GET"))
            .and(path("/services/data/v67.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(soql_response))
            .mount(&mock_server)
            .await;

        let md = generate_visualizer_report(&client, "Account", true)
            .await
            .must();

        assert!(md.contains("# Schema Report: Account"));
        assert!(md.contains("## Field Usage Statistics"));
    }
}
