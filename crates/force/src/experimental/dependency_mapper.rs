//! SObject Dependency Mapper.
//!
//! This module provides an experimental utility to map out the dependency graph
//! of an SObject. It answers the question: "If I want to copy or migrate this SObject,
//! what other objects must I migrate with it?"
//!
//! This serves as a powerful "Exporter/Mashup" feature that allows scanning references
//! (Lookups/Master-Detail) recursively up to a certain depth.
//!
//! # Example
//!
//! ```no_run
//! # use force::client::ForceClientBuilder;
//! # use force::experimental::SObjectDependencyMapper;
//! # use force::auth::ClientCredentials;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let auth = ClientCredentials::new("id", "secret", "url");
//! # let client = ForceClientBuilder::new().authenticate(auth).build().await?;
//! let mut mapper = SObjectDependencyMapper::new(&client);
//! let dependencies = mapper.map_dependencies("Contact", 2).await?;
//!
//! println!("Found {} dependencies", dependencies.len());
//! for dep in dependencies {
//!     println!("{} -> {}", dep.source_object, dep.target_object);
//! }
//! # Ok(())
//! # }
//! ```

use crate::api::rest::describe::{FieldType, SObjectDescribe};
use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::{HashSet, VecDeque};

/// Represents a dependency between two SObjects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    /// The API name of the source SObject (e.g., "Contact").
    pub source_object: String,
    /// The API name of the field on the source object (e.g., "AccountId").
    pub field_name: String,
    /// The API name of the target SObject (e.g., "Account").
    pub target_object: String,
    /// Whether this field is required (i.e., not nillable).
    pub is_required: bool,
    /// The depth of this dependency from the root SObject.
    pub depth: u32,
}

/// Mapper for traversing and discovering SObject dependencies.
#[derive(Debug)]
pub struct SObjectDependencyMapper<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
    // Pre-cache describe results to avoid redundant API calls
    cache: std::collections::HashMap<String, SObjectDescribe>,
}

impl<'a, A: Authenticator> SObjectDependencyMapper<'a, A> {
    /// Creates a new dependency mapper.
    ///
    /// # Arguments
    ///
    /// * `client` - The authenticated Force client.
    #[must_use]
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self {
            client,
            cache: std::collections::HashMap::new(),
        }
    }

    /// Recursively maps dependencies for a given root SObject.
    ///
    /// # Arguments
    ///
    /// * `root_sobject` - The API name of the starting SObject.
    /// * `max_depth` - The maximum depth to traverse (0 = no traversal, just the object itself,
    ///   but typically this returns dependencies, so 1 means direct parents).
    ///
    /// # Returns
    ///
    /// A list of dependency nodes.
    pub async fn map_dependencies(
        &mut self,
        root_sobject: &str,
        max_depth: u32,
    ) -> Result<Vec<DependencyNode>> {
        let mut dependencies = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((root_sobject.to_string(), 1));
        visited.insert(root_sobject.to_string());

        while let Some((current_sobject, current_depth)) = queue.pop_front() {
            if current_depth > max_depth {
                continue;
            }

            let describe = self.fetch_describe(&current_sobject).await?;

            for field in &describe.fields {
                if field.type_ == FieldType::Reference {
                    for target in &field.reference_to {
                        // Avoid self-references or revisiting the same target at a shallower depth
                        // (for simplicity, we just add the dependency and optionally queue it)
                        let dep = DependencyNode {
                            source_object: current_sobject.clone(),
                            field_name: field.name.clone(),
                            target_object: target.clone(),
                            is_required: !field.nillable,
                            depth: current_depth,
                        };
                        dependencies.push(dep);

                        if !visited.contains(target) {
                            visited.insert(target.clone());
                            queue.push_back((target.clone(), current_depth + 1));
                        }
                    }
                }
            }
        }

        Ok(dependencies)
    }

    async fn fetch_describe(&mut self, sobject: &str) -> Result<SObjectDescribe> {
        if let Some(describe) = self.cache.get(sobject) {
            return Ok(describe.clone());
        }

        let describe = self.client.rest().describe(sobject).await?;
        self.cache.insert(sobject.to_string(), describe.clone());
        Ok(describe)
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

    async fn setup_mock_describe_contact(mock_server: &MockServer) {
        let id_field = json!({
            "name": "Id", "type": "id", "label": "Contact ID",
            "referenceTo": [],
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
            "nillable": true,
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
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

    async fn setup_mock_describe_account(mock_server: &MockServer) {
        let id_field = json!({
            "name": "Id", "type": "id", "label": "Account ID",
            "referenceTo": [],
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

        let owner_id_field = json!({
            "name": "OwnerId", "type": "reference", "label": "Owner ID",
            "referenceTo": ["User"],
            "nillable": false,
            "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
            "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
            "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
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
            "fields": [id_field, owner_id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(mock_server)
            .await;
    }

    async fn setup_mock_describe_user(mock_server: &MockServer) {
        let id_field = json!({
            "name": "Id", "type": "id", "label": "User ID",
            "referenceTo": [],
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
            "name": "User",
            "label": "User",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "005", "labelPlural": "Users", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/User/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(describe_json))
            .mount(mock_server)
            .await;
    }

    #[tokio::test]
    async fn test_map_dependencies_depth_1() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        setup_mock_describe_contact(&mock_server).await;

        let mut mapper = SObjectDependencyMapper::new(&client);
        let deps = mapper.map_dependencies("Contact", 1).await.must();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].source_object, "Contact");
        assert_eq!(deps[0].field_name, "AccountId");
        assert_eq!(deps[0].target_object, "Account");
        assert_eq!(deps[0].depth, 1);
        assert!(!deps[0].is_required); // nillable = true
    }

    #[tokio::test]
    async fn test_map_dependencies_depth_2() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        setup_mock_describe_contact(&mock_server).await;
        setup_mock_describe_account(&mock_server).await;
        setup_mock_describe_user(&mock_server).await;

        let mut mapper = SObjectDependencyMapper::new(&client);
        let deps = mapper.map_dependencies("Contact", 2).await.must();

        assert_eq!(deps.len(), 2);

        let account_dep = deps.iter().find(|d| d.target_object == "Account").unwrap();
        assert_eq!(account_dep.source_object, "Contact");
        assert_eq!(account_dep.depth, 1);

        let user_dep = deps.iter().find(|d| d.target_object == "User").unwrap();
        assert_eq!(user_dep.source_object, "Account");
        assert_eq!(user_dep.field_name, "OwnerId");
        assert_eq!(user_dep.depth, 2);
        assert!(user_dep.is_required); // nillable = false
    }
}
