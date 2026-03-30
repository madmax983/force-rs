//! Dependency Mapper for Salesforce SObjects.
//!
//! This module provides `SObjectDependencyMapper`, a utility to map out the dependency
//! graph of a Salesforce SObject using Breadth-First Search (BFS). It traces foreign
//! keys (lookup/master-detail relationships) up to a specified depth. This is useful
//! for understanding complex object hierarchies and determining the order of operations
//! for data insertion/deletion.
//!
//! # The Spark
//! We have schema discovery tools (`SchemaGraph`, `DataDictionary`), but they mostly build
//! metadata graphs. What if we could use that metadata to figure out exactly *what order*
//! we need to insert records? `DependencyMapper` connects SObject relationships to actionable
//! execution plans.

use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::{HashMap, HashSet, VecDeque};

/// A node in the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    /// The API name of the SObject.
    pub name: String,
    /// The API names of the SObjects this node depends on (foreign keys).
    pub depends_on: HashSet<String>,
}

/// Mapper for SObject dependencies.
pub struct SObjectDependencyMapper<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    nodes: HashMap<String, DependencyNode>,
}

impl<'a, A: Authenticator> SObjectDependencyMapper<'a, A> {
    /// Creates a new dependency mapper.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self {
            client,
            nodes: HashMap::new(),
        }
    }

    /// Maps the dependencies of the specified SObject up to a maximum depth.
    ///
    /// # Arguments
    ///
    /// * `root_sobject` - The starting SObject API name.
    /// * `max_depth` - The maximum depth to traverse (e.g., 2 means find parents, and parents of parents).
    pub async fn map_dependencies(&mut self, root_sobject: &str, max_depth: usize) -> Result<()> {
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        queue.push_back((root_sobject.to_string(), 0));

        while let Some((current_sobject, current_depth)) = queue.pop_front() {
            if current_depth > max_depth || visited.contains(&current_sobject) {
                continue;
            }

            visited.insert(current_sobject.clone());

            let describe = self.client.rest().describe(&current_sobject).await?;
            let mut depends_on = HashSet::new();

            for field in describe.fields {
                if !field.reference_to.is_empty() {
                    for ref_obj in field.reference_to {
                        depends_on.insert(ref_obj.clone());
                        if current_depth < max_depth {
                            queue.push_back((ref_obj, current_depth + 1));
                        }
                    }
                }
            }

            self.nodes.insert(
                current_sobject.clone(),
                DependencyNode {
                    name: current_sobject,
                    depends_on,
                },
            );
        }

        Ok(())
    }

    /// Returns the mapped nodes.
    #[must_use]
    pub fn get_nodes(&self) -> &HashMap<String, DependencyNode> {
        &self.nodes
    }

    /// Returns an ordered list of SObjects suitable for insertion.
    /// SObjects with no dependencies come first.
    #[must_use]
    pub fn insertion_order(&self) -> Vec<String> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        // Use a sorted vector of keys to ensure deterministic output
        let mut keys: Vec<&String> = self.nodes.keys().collect();
        keys.sort();

        for node_name in keys {
            self.topological_sort(node_name, &mut visited, &mut visiting, &mut order);
        }

        order
    }

    fn topological_sort(
        &self,
        node_name: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) {
        if visited.contains(node_name) || visiting.contains(node_name) {
            return;
        }

        visiting.insert(node_name.to_string());

        if let Some(node) = self.nodes.get(node_name) {
            // Sort dependencies to ensure deterministic output
            let mut deps: Vec<&String> = node.depends_on.iter().collect();
            deps.sort();

            for dep in deps {
                self.topological_sort(dep, visited, visiting, order);
            }
        }

        visiting.remove(node_name);
        visited.insert(node_name.to_string());
        order.push(node_name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::rest::describe::{FieldDescribe, FieldType, SObjectDescribe};
    use crate::client::ForceClientBuilder;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use std::collections::HashMap;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_describe(name: &str, reference_to: Vec<&str>) -> SObjectDescribe {
        let fields = reference_to
            .into_iter()
            .map(|r| FieldDescribe {
                aggregatable: true,
                auto_number: false,
                byte_length: 18,
                calculated: false,
                calculated_formula: None,
                cascade_delete: false,
                case_sensitive: false,
                compound_field_name: None,
                controller_name: None,
                createable: true,
                custom: false,
                defaulted_on_create: false,
                default_value: None,
                default_value_formula: None,
                formula_treat_blanks_as: None,
                dependent_picklist: false,
                deprecated_and_hidden: false,
                digits: 0,
                display_location_in_decimal: false,
                encrypted: false,
                external_id: false,
                extra_type_info: None,
                filterable: true,
                filtered_lookup_info: None,

                groupable: true,
                high_scale_number: false,
                html_formatted: false,
                id_lookup: false,
                inline_help_text: None,
                label: "Ref".to_string(),
                length: 18,
                mask: None,
                mask_type: None,
                name: "RefId".to_string(),
                name_field: false,
                name_pointing: false,
                nillable: true,
                permissionable: true,
                picklist_values: None,
                polymorphic_foreign_key: false,
                precision: 0,
                query_by_distance: false,
                reference_target_field: None,
                reference_to: vec![r.to_string()],
                relationship_name: None,
                relationship_order: None,
                restricted_delete: false,
                restricted_picklist: false,
                scale: 0,
                search_prefixes_supported: None,
                soap_type: "tns:ID".to_string(),
                sortable: true,
                type_: FieldType::Reference,
                unique: false,
                updateable: true,
                write_requires_master_read: false,
            })
            .collect();

        SObjectDescribe {
            activateable: false,
            createable: true,
            custom: false,
            custom_setting: false,
            deletable: true,
            deprecated_and_hidden: false,
            feed_enabled: false,
            has_subtypes: false,
            is_subtype: false,
            key_prefix: None,
            label: name.to_string(),
            label_plural: format!("{}s", name),
            layoutable: true,
            mergeable: false,
            mru_enabled: false,
            name: name.to_string(),
            queryable: true,
            replicateable: true,
            retrieveable: true,
            searchable: true,
            triggerable: true,
            undeletable: true,
            updateable: true,
            fields,
            child_relationships: vec![],
            record_type_infos: vec![],
            urls: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_dependency_mapper_order() {
        let server = MockServer::start().await;

        let contact_describe = mock_describe("Contact", vec!["Account"]);
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Contact/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&contact_describe))
            .mount(&server)
            .await;

        let account_describe = mock_describe("Account", vec![]);
        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&account_describe))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/services/oauth2/userinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "organization_id": "00D...", "user_id": "005..." }),
            ))
            .mount(&server)
            .await;

        let auth = MockAuthenticator::new("my_token", &server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let mut mapper = SObjectDependencyMapper::new(&client);
        mapper.map_dependencies("Contact", 1).await.must();

        let order = mapper.insertion_order();

        // Account should be inserted before Contact because Contact depends on Account
        assert_eq!(order, vec!["Account", "Contact"]);
    }
}
