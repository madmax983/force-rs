use crate::api::rest_operation::RestOperation;
use crate::auth::Authenticator;
use crate::client::ForceClient;
use crate::error::Result;
use std::collections::{HashMap, HashSet, VecDeque};

/// A node in the dependency graph representing an SObject and its relationship depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    /// The API name of the SObject (e.g., "Account").
    pub name: String,
    /// The shortest path depth from the root SObject (0 for root).
    pub depth: usize,
    /// SObjects that this object references (foreign keys).
    pub references: HashSet<String>,
}

/// A utility to map out the dependency graph (foreign keys/lookups) of a Salesforce SObject
/// up to a specified depth using Breadth-First Search (BFS).
pub struct SObjectDependencyMapper<'a, A: Authenticator> {
    client: &'a ForceClient<A>,
}

impl<'a, A: Authenticator> SObjectDependencyMapper<'a, A> {
    /// Creates a new `SObjectDependencyMapper`.
    pub fn new(client: &'a ForceClient<A>) -> Self {
        Self { client }
    }

    /// Maps dependencies for a root SObject up to a maximum depth.
    /// Returns a map of SObject API names to their DependencyNode details.
    pub async fn map_dependencies(
        &self,
        root_sobject: &str,
        max_depth: usize,
    ) -> Result<HashMap<String, DependencyNode>> {
        let mut graph: HashMap<String, DependencyNode> = HashMap::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        queue.push_back((root_sobject.to_string(), 0));

        while let Some((current_sobject, current_depth)) = queue.pop_front() {
            // If we've already processed this object at a shallower or equal depth, skip it
            if let Some(existing) = graph.get(&current_sobject) {
                if existing.depth <= current_depth {
                    continue;
                }
            }

            // Fetch the describe metadata to find references
            let describe = self.client.rest().describe(&current_sobject).await?;
            let mut references = HashSet::new();

            for field in describe.fields {
                for reference_target in field.reference_to {
                    references.insert(reference_target);
                }
            }

            // Record the current node
            graph.insert(
                current_sobject.clone(),
                DependencyNode {
                    name: current_sobject.clone(),
                    depth: current_depth,
                    references: references.clone(),
                },
            );

            // If we haven't hit the depth limit, enqueue the references
            if current_depth < max_depth {
                for reference_target in references {
                    queue.push_back((reference_target, current_depth + 1));
                }
            }
        }

        Ok(graph)
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

    #[tokio::test]
    async fn test_map_dependencies() {
        let mock_server = create_mock_server().await;
        let client = create_test_client(&mock_server).await;

        let account_id_field = json!({
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

        let account_owner_id_field = json!({
            "name": "OwnerId", "type": "reference", "label": "Owner ID",
            "referenceTo": ["User"],
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

        let account_describe = json!({
            "name": "Account", "label": "Account", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [account_id_field, account_owner_id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/Account/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(account_describe))
            .mount(&mock_server)
            .await;

        let user_id_field = json!({
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

        let user_describe = json!({
            "name": "User", "label": "User", "custom": false, "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "005", "labelPlural": "Users", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": [user_id_field]
        });

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/sobjects/User/describe"))
            .respond_with(ResponseTemplate::new(200).set_body_json(user_describe))
            .mount(&mock_server)
            .await;

        let mapper = SObjectDependencyMapper::new(&client);
        let deps = mapper.map_dependencies("Account", 2).await.must();

        assert_eq!(deps.len(), 2);

        let account_node = deps.get("Account").must();
        assert_eq!(account_node.depth, 0);
        assert!(account_node.references.contains("User"));

        let user_node = deps.get("User").must();
        assert_eq!(user_node.depth, 1);
        assert!(user_node.references.is_empty());
    }
}
