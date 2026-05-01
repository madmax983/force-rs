//! Relational Data Seeder for rapid testing and development.
//!
//! This module provides `RelationalSeeder`, a utility that combines the schema discovery
//! powers of `DataFaker` with the relational capabilities of `CompositeGraph` to generate and
//! insert mock records with parent-child relationships into Salesforce in a single atomic call.
//!
//! # The Spark
//! We have `DataSeeder` for flat objects, and `CompositeGraph` for relational inserts.
//! By automatically discovering lookup fields via SObject Describes, we can generate a full
//! Parent-Child hierarchy (e.g., Account + Contact) without manual foreign key wiring!

use crate::api::composite::Graph;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::{ForceError, Result};
use crate::types::describe::{FieldType, SObjectDescribe};

use super::data_faker::generate_mock_record;

/// Utility for generating and inserting relational mock records from schema metadata.
#[derive(Debug)]
pub struct RelationalSeeder<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> RelationalSeeder<'a, A> {
    /// Creates a new relational seeder.
    ///
    /// # Arguments
    ///
    /// * `client` - The Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Generates and inserts a parent record and multiple child records.
    ///
    /// This method generates mock data for the parent and child objects using `DataFaker`.
    /// It auto-discovers the relationship field on the child object that points to the parent.
    /// Then, it uses the Composite Graph API to insert all records atomically, automatically
    /// resolving the foreign keys.
    ///
    /// # Arguments
    ///
    /// * `parent_describe` - The schema describe of the parent object.
    /// * `child_describe` - The schema describe of the child object.
    /// * `child_count` - The number of child records to generate for the parent.
    ///
    /// # Errors
    ///
    /// Returns an error if no relationship field is found between the child and parent,
    /// or if the Composite Graph execution fails.
    pub async fn seed_hierarchy(
        &self,
        parent_describe: &SObjectDescribe,
        child_describe: &SObjectDescribe,
        child_count: usize,
    ) -> Result<()> {
        // 1. Discover the relationship field on the child pointing to the parent.
        let relationship_field = child_describe.fields.iter().find(|f| {
            f.type_ == FieldType::Reference && f.reference_to.contains(&parent_describe.name)
        });

        let Some(rel_field) = relationship_field else {
            return Err(ForceError::InvalidInput(format!(
                "No reference field found on {} pointing to {}",
                child_describe.name, parent_describe.name
            )));
        };

        // 2. Generate the Parent record.
        let mut parent_record = generate_mock_record(parent_describe);
        let parent_ref_id = format!("ref_{}", parent_describe.name.to_lowercase());

        let mut graph = Graph::new("SeedGraph");

        // Remove 'Id' if present, as we are creating a new record.
        parent_record.fields.remove("Id");

        let parent_value = serde_json::to_value(&parent_record.fields)
            .map_err(|e| ForceError::Serialization(crate::error::SerializationError::Json(e)))?;

        graph = graph.post(&parent_describe.name, parent_value, &parent_ref_id)?;

        // 3. Generate Child records and link them to the Parent.
        let parent_id_ref_str = format!("@{{ {}.id }}", parent_ref_id).replace(" ", "");

        for i in 0..child_count {
            let mut child_record = generate_mock_record(child_describe);

            child_record.fields.remove("Id");

            // Wire up the foreign key.
            child_record.fields.insert(
                rel_field.name.clone(),
                serde_json::Value::String(parent_id_ref_str.clone()),
            );

            let child_ref_id = format!("ref_{}_{}", child_describe.name.to_lowercase(), i);

            let child_value = serde_json::to_value(&child_record.fields).map_err(|e| {
                ForceError::Serialization(crate::error::SerializationError::Json(e))
            })?;

            graph = graph.post(&child_describe.name, child_value, &child_ref_id)?;
        }

        // 4. Execute the Graph.
        let mut graph_req = self.client.composite().graph();
        graph_req = graph_req.add_graph(graph)?;
        let response = graph_req.execute().await?;

        for res in response.graphs {
            if !res.is_successful {
                return Err(ForceError::InvalidInput("Seed operation failed".into()));
            }
        }

        Ok(())
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

    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }

    async fn create_test_client(mock_server: &MockServer) -> ForceClient<MockAuthenticator> {
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        builder().authenticate(auth).build().await.must()
    }

    fn create_mock_describe(name: &str, fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = json!({
            "name": name,
            "label": name,
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": format!("{}s", name), "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(name: &str, field_type: &str, ref_to: &[&str]) -> serde_json::Value {
        json!({
            "name": name,
            "type": field_type,
            "label": format!("{} Label", name),
            "createable": true,
            "autoNumber": false, "calculated": false,
            "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false, "nillable": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "xsd:string",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false,
            "referenceTo": ref_to
        })
    }

    #[tokio::test]
    async fn test_seed_hierarchy_success() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let parent_describe =
            create_mock_describe("Account", &json!([mock_field("Name", "string", &[])]));

        let child_describe = create_mock_describe(
            "Contact",
            &json!([
                mock_field("LastName", "string", &[]),
                mock_field("AccountId", "reference", &["Account"])
            ]),
        );

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/composite/graph"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "graphs": [
                    {
                        "graphId": "SeedGraph",
                        "isSuccessful": true,
                        "compositeResponse": [
                            {
                                "body": { "id": "001000000000001AAA", "success": true, "errors": [] },
                                "httpHeaders": {}, "httpStatusCode": 201, "referenceId": "ref_account"
                            },
                            {
                                "body": { "id": "003000000000001AAA", "success": true, "errors": [] },
                                "httpHeaders": {}, "httpStatusCode": 201, "referenceId": "ref_contact_0"
                            }
                        ]
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let seeder = RelationalSeeder::new(&client);
        let result = seeder
            .seed_hierarchy(&parent_describe, &child_describe, 1)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_seed_hierarchy_missing_relationship() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let parent_describe =
            create_mock_describe("Account", &json!([mock_field("Name", "string", &[])]));

        // Child object has NO reference to Account
        let child_describe = create_mock_describe(
            "Contact",
            &json!([
                mock_field("LastName", "string", &[]),
                mock_field("OpportunityId", "reference", &["Opportunity"])
            ]),
        );

        let seeder = RelationalSeeder::new(&client);
        let result = seeder
            .seed_hierarchy(&parent_describe, &child_describe, 1)
            .await;

        assert!(matches!(result, Err(ForceError::InvalidInput(_))));
        if let Err(ForceError::InvalidInput(msg)) = result {
            assert!(msg.contains("No reference field found"));
        }
    }
}
