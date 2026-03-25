//! Salesforce Schema Graph Visualizer.
//!
//! This module provides a utility to scan SObject metadata and generate
//! Mermaid.js Entity Relationship (ER) diagrams. It helps developers
//! visualize complex object relationships and dependencies.
//!
//! # Example
//!
//! ```no_run
//! # use force::client::ForceClientBuilder;
//! # use force::experimental::SchemaGraph;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let mut graph = SchemaGraph::new(&client);
//! graph.scan("Account").await?;
//! graph.scan("Contact").await?;
//!
//! println!("{}", graph.to_mermaid());
//! // erDiagram
//! //   Account ||--o{ Contact : "AccountId"
//! # Ok(())
//! # }
//! ```

use crate::api::rest::describe::{FieldType, SObjectDescribe};
use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// A node in the schema graph representing an SObject.
#[derive(Debug, Clone)]
struct SchemaNode {
    name: String,
    label: String,
    fields: Vec<SchemaField>,
}

/// A field in a schema node.
#[derive(Debug, Clone)]
struct SchemaField {
    name: String,
    type_: FieldType,
    reference_to: Vec<String>,
}

/// Graph builder for Salesforce schema visualization.
#[derive(Debug)]
pub struct SchemaGraph<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    nodes: HashMap<String, SchemaNode>,
    scanned: HashSet<String>,
}

impl<'a, A: Authenticator> SchemaGraph<'a, A> {
    /// Creates a new schema graph builder.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self {
            client,
            nodes: HashMap::new(),
            scanned: HashSet::new(),
        }
    }

    /// Scans an SObject and adds it to the graph.
    ///
    /// Fetches metadata using the Describe API and records fields and relationships.
    /// Use this method multiple times to build a graph of related objects.
    pub async fn scan(&mut self, sobject: &str) -> Result<()> {
        if !self.scanned.insert(sobject.to_string()) {
            return Ok(()); // Already scanned — skip redundant API call
        }
        let describe = self.client.rest().describe(sobject).await?;
        self.add_node(describe);
        Ok(())
    }

    /// Adds a node from an already-fetched describe result.
    ///
    /// Use this to avoid redundant API calls when you already have the describe data.
    pub fn add_describe(&mut self, describe: SObjectDescribe) {
        self.scanned.insert(describe.name.clone());
        self.add_node(describe);
    }

    /// Adds a node from a describe result (internal helper).
    fn add_node(&mut self, describe: SObjectDescribe) {
        let node = SchemaNode {
            name: describe.name.clone(),
            label: describe.label,
            fields: describe
                .fields
                .into_iter()
                .map(|f| SchemaField {
                    name: f.name,
                    type_: f.type_,
                    reference_to: f.reference_to,
                })
                .collect(),
        };
        self.nodes.insert(describe.name, node);
    }

    /// Generates a Mermaid.js ER diagram from the scanned objects.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("erDiagram\n");

        // Sort nodes for deterministic output
        let mut node_names: Vec<_> = self.nodes.keys().collect();
        node_names.sort();

        // 1. Define entities and fields
        for name in &node_names {
            let node = &self.nodes[*name];
            let _ = writeln!(mermaid, "    {} {{", node.name);

            for field in &node.fields {
                let type_str = match field.type_ {
                    FieldType::Int => "int",
                    FieldType::Double => "double",
                    FieldType::Boolean => "boolean",
                    FieldType::Id => "id",
                    FieldType::Reference => "reference",
                    FieldType::Date => "date",
                    FieldType::Datetime => "datetime",
                    FieldType::Picklist => "picklist",
                    _ => "string", // Simplify other types for visualization
                };
                let _ = writeln!(mermaid, "        {} {}", type_str, field.name);
            }
            mermaid.push_str("    }\n\n");
        }

        // 2. Define relationships
        for name in &node_names {
            let node = &self.nodes[*name];
            for field in &node.fields {
                if field.type_ == FieldType::Reference {
                    for target in &field.reference_to {
                        // Only draw edge if target is also in graph (to avoid dangling edges)
                        if self.nodes.contains_key(target) {
                            // Relationship: Target ||--o{ Source : FieldName
                            // Example: Account ||--o{ Contact : AccountId
                            let _ = writeln!(
                                mermaid,
                                "    {} ||--o{{ {} : \"{}\"",
                                target, node.name, field.name
                            );
                        }
                    }
                }
            }
        }

        mermaid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_mock_describe_account(mock_server: &MockServer) {
        // Construct fields separately to avoid recursion limit
        let id_field = json!({
            "name": "Id", "type": "id", "label": "Account ID",
            "referenceTo": [],
            // Mandatory fields filler
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        });

        let name_field = json!({
            "name": "Name", "type": "string", "label": "Account Name",
            "referenceTo": [],
            // Mandatory fields filler
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        });

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
            "fields": [id_field, name_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(mock_server)
            .await;
    }

    async fn setup_mock_describe_contact(mock_server: &MockServer) {
        // Construct fields separately to avoid recursion limit
        let id_field = json!({
            "name": "Id", "type": "id", "label": "Contact ID",
            "referenceTo": [],
            // Mandatory fields filler
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        });

        let account_id_field = json!({
            "name": "AccountId", "type": "reference", "label": "Account ID",
            "referenceTo": ["Account"],
            // Mandatory fields filler
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        });

        let describe_json = json!({
            "name": "Contact",
            "label": "Contact",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "003", "labelPlural": "Contacts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [id_field, account_id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(mock_server)
            .await;
    }

    #[tokio::test]
    async fn test_schema_graph_generation() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        setup_mock_describe_account(&mock_server).await;
        setup_mock_describe_contact(&mock_server).await;

        let mut graph = SchemaGraph::new(&client);
        graph.scan("Account").await.must();
        graph.scan("Contact").await.must();

        let mermaid = graph.to_mermaid();

        println!("Generated Mermaid:\n{}", mermaid);

        // Assert basic structure
        assert!(mermaid.contains("erDiagram"));

        // Assert entities exist
        assert!(mermaid.contains("Account {"));
        assert!(mermaid.contains("Contact {"));

        // Assert fields exist
        assert!(mermaid.contains("string Name"));
        assert!(mermaid.contains("id Id"));

        // Assert relationship exists
        // Contact has AccountId reference to Account
        // So: Account ||--o{ Contact : "AccountId"
        assert!(mermaid.contains("Account ||--o{ Contact : \"AccountId\""));
    }
}
