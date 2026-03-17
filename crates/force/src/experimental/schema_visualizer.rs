use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use crate::experimental::scanner::FieldUsageScanner;
use crate::experimental::schema_analyzer::SchemaAnalyzer;
use crate::experimental::schema_graph::SchemaGraph;
use std::collections::HashMap;

/// A utility to generate a comprehensive Markdown report of an SObject schema.
///
/// This combines `SchemaAnalyzer`, `SchemaGraph`, and `FieldUsageScanner`
/// into a single, unified visual report.
#[derive(Debug)]
pub struct SchemaVisualizer<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> SchemaVisualizer<'a, A> {
    /// Creates a new `SchemaVisualizer` instance.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Generates a comprehensive Markdown report for the given SObject.
    ///
    /// The report includes:
    /// - Schema Insights (Complexity score, field counts, etc.)
    /// - ER Diagram (Mermaid.js)
    /// - Field Usage Statistics (Optional)
    ///
    /// # Arguments
    ///
    /// * `sobject` - The API name of the SObject (e.g., "Account").
    /// * `include_usage` - Whether to scan and include field population statistics.
    pub async fn generate_report(&self, sobject: &str, include_usage: bool) -> Result<String> {
        let describe = self.client.rest().describe(sobject).await?;

        let analyzer = SchemaAnalyzer::new();
        let insights = analyzer.analyze(&describe);

        let mut graph = SchemaGraph::new(self.client);
        graph.scan(sobject).await?;
        let mermaid = graph.to_mermaid();

        let mut md = String::with_capacity(2048);

        md.push_str(&format!("# Schema Report: {}\n\n", describe.label));
        md.push_str(&format!("**API Name:** `{}`\n", describe.name));
        md.push_str(&format!("**Custom:** {}\n\n", describe.custom));

        md.push_str("## Schema Insights\n\n");
        md.push_str(&format!(
            "*   **Complexity Score:** {}\n",
            insights.complexity_score
        ));
        md.push_str(&format!(
            "*   **Total Fields:** {}\n",
            insights.total_fields
        ));
        md.push_str(&format!(
            "*   **Standard Fields:** {}\n",
            insights.standard_field_count
        ));
        md.push_str(&format!(
            "*   **Custom Fields:** {}\n",
            insights.custom_field_count
        ));
        md.push_str(&format!(
            "*   **Required Fields:** {}\n\n",
            insights.required_field_count
        ));

        md.push_str("## Entity-Relationship Diagram\n\n");
        md.push_str("```mermaid\n");
        md.push_str(&mermaid);
        md.push_str("```\n\n");

        if include_usage {
            let scanner = FieldUsageScanner::new(self.client);
            let usages = scanner.scan(sobject).await?;
            let mut usage_map = HashMap::new();
            for usage in usages {
                usage_map.insert(usage.name.clone(), usage);
            }

            md.push_str("## Field Usage Statistics\n\n");
            md.push_str("| Label | API Name | Populated % |\n");
            md.push_str("|---|---|---|\n");

            let mut fields = describe.fields;
            fields.sort_by(|a, b| a.name.cmp(&b.name));

            for field in &fields {
                let pop_pct = if let Some(u) = usage_map.get(&field.name) {
                    format!("{:.1}%", u.percentage)
                } else {
                    "N/A".to_string()
                };

                md.push_str(&format!(
                    "| {} | `{}` | {} |\n",
                    field.label, field.name, pop_pct
                ));
            }
        }

        Ok(md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
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
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(&mock_server)
            .await;

        let visualizer = SchemaVisualizer::new(&client);
        let md = visualizer.generate_report("Account", false).await.must();

        assert!(md.contains("# Schema Report: Account"));
        assert!(md.contains("**API Name:** `Account`"));
        assert!(md.contains("## Schema Insights"));
        assert!(md.contains("## Entity-Relationship Diagram"));
        assert!(md.contains("```mermaid"));
        assert!(md.contains("erDiagram"));
        // Check that usage statistics section is absent since include_usage is false
        assert!(!md.contains("## Field Usage Statistics"));
    }
}
