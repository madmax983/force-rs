//! Tooling API `symbols` resource (Beta, v68.0+).
//!
//! Returns Apex type metadata: classes, interfaces, enums, triggers, and their
//! members. The org allows one request at a time. Do not call it in parallel.

use crate::api::rest_operation::RestOperation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Category of Apex types to get.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolCategory {
    /// Standard Apex types (`System`, `Database`, `Messaging`, ...).
    Builtin,
    /// Custom and packaged Apex types.
    Database,
    /// Dynamic Apex types.
    Dynamic,
}

impl SymbolCategory {
    /// Returns the wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Database => "database",
            Self::Dynamic => "dynamic",
        }
    }
}

/// Query for the `symbols` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolsRequest {
    /// Required category.
    pub category: SymbolCategory,
    /// Namespace filter. `Some("")` means the local org namespace only.
    /// `None` means all namespaces.
    pub namespace: Option<String>,
    /// Type name filter.
    pub name: Option<String>,
}

impl SymbolsRequest {
    /// Creates a request for all types in `category`.
    #[must_use]
    pub const fn new(category: SymbolCategory) -> Self {
        Self {
            category,
            namespace: None,
            name: None,
        }
    }

    /// Sets the namespace filter. Use `""` for the local namespace.
    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Sets the type name filter.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Returns the query parameters. An empty namespace is kept.
    fn query_params(&self) -> Vec<(&'static str, &str)> {
        let mut params = vec![("category", self.category.as_str())];
        if let Some(ns) = &self.namespace {
            params.push(("namespace", ns.as_str()));
        }
        if let Some(name) = &self.name {
            params.push(("name", name.as_str()));
        }
        params
    }
}

/// Reference to an Apex type, for example `List<String>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TypeRef {
    /// Namespace, or `None`.
    #[serde(default)]
    pub namespace_prefix: Option<String>,
    /// Type name.
    #[serde(default)]
    pub name: String,
    /// Generic type arguments, or `None` if not generic.
    #[serde(default)]
    pub type_parameters: Option<Vec<Self>>,
}

/// Parameter of an annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationParameter {
    /// Parameter name.
    #[serde(default)]
    pub name: String,
    /// Parameter type.
    #[serde(default, rename = "type")]
    pub param_type: Option<TypeRef>,
    /// Parameter value.
    #[serde(default)]
    pub value: Option<String>,
}

/// Annotation on a type or member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// Annotation name.
    #[serde(default)]
    pub name: String,
    /// Annotation parameters.
    #[serde(default)]
    pub parameters: Vec<AnnotationParameter>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
}

/// Field declared on a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSymbol {
    /// Field name.
    #[serde(default)]
    pub name: String,
    /// Field type.
    #[serde(default, rename = "type")]
    pub field_type: Option<TypeRef>,
    /// Modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
    /// Declaring type. Set only for inherited fields.
    #[serde(default)]
    pub defining_type: Option<TypeRef>,
}

/// Property accessor (`get` or `set`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accessor {
    /// Modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
}

/// Property declared on a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertySymbol {
    /// Property name.
    #[serde(default)]
    pub name: String,
    /// Property type.
    #[serde(default, rename = "type")]
    pub property_type: Option<TypeRef>,
    /// Modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Get accessor, if present.
    #[serde(default)]
    pub getter: Option<Accessor>,
    /// Set accessor, if present.
    #[serde(default)]
    pub setter: Option<Accessor>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
    /// Declaring type. Set only for inherited properties.
    #[serde(default)]
    pub defining_type: Option<TypeRef>,
}

/// Method or constructor parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSymbol {
    /// Parameter name.
    #[serde(default)]
    pub name: String,
    /// Parameter type.
    #[serde(default, rename = "type")]
    pub param_type: Option<TypeRef>,
    /// Annotations.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
}

/// Method or constructor declared on a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodSymbol {
    /// Method name.
    #[serde(default)]
    pub name: String,
    /// `Some(true)` for a constructor. Otherwise `None`.
    #[serde(default)]
    pub is_constructor: Option<bool>,
    /// Return type. `None` for a constructor.
    #[serde(default)]
    pub return_type: Option<TypeRef>,
    /// Modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Parameters.
    #[serde(default)]
    pub parameters: Vec<ParameterSymbol>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
    /// Declaring type. Set only for inherited methods.
    #[serde(default)]
    pub defining_type: Option<TypeRef>,
}

/// Apex type stub: class, interface, enum, or trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStub {
    /// Type name.
    #[serde(default)]
    pub name: String,
    /// Namespace, or `None`.
    #[serde(default)]
    pub namespace_prefix: Option<String>,
    /// `CLASS`, `INTERFACE`, `ENUM`, or `TRIGGER`.
    #[serde(default)]
    pub kind: String,
    /// Modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Superclass, or `None`.
    #[serde(default)]
    pub super_class: Option<TypeRef>,
    /// Implemented interfaces.
    #[serde(default)]
    pub interfaces: Vec<TypeRef>,
    /// Fields.
    #[serde(default)]
    pub fields: Vec<FieldSymbol>,
    /// Properties.
    #[serde(default)]
    pub properties: Vec<PropertySymbol>,
    /// Methods and constructors.
    #[serde(default)]
    pub methods: Vec<MethodSymbol>,
    /// Nested types.
    #[serde(default)]
    pub inner_types: Vec<Self>,
    /// Trigger operations, for example `BEFORE UPDATE`. `None` for non-triggers.
    #[serde(default)]
    pub trigger_operations: Option<Vec<String>>,
    /// Documentation.
    #[serde(default)]
    pub documentation: Option<String>,
    /// Trigger sObject. `None` for non-triggers.
    #[serde(default)]
    pub trigger_object_type: Option<TypeRef>,
    /// Compile error text, or `None`.
    #[serde(default)]
    pub compile_error: Option<String>,
    /// Fields that this crate does not model.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Response of the `symbols` resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolsResult {
    /// Type stubs.
    #[serde(default)]
    pub type_stubs: Vec<TypeStub>,
}

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Gets Apex type metadata (Beta).
    ///
    /// `GET /tooling/symbols/`. Needs API v68.0+, Author Apex, and View Setup.
    /// The org allows one request at a time. A parallel request fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be decoded.
    pub async fn symbols(&self, request: &SymbolsRequest) -> crate::error::Result<SymbolsResult> {
        let url = self.session().resolve_url("tooling/symbols/").await?;
        let http_request = self
            .session()
            .get(&url)
            .query(&request.query_params())
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(http_request, "Symbols request failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_utils::mock_auth::MockAuthenticator;
    use crate::test_utils::must::Must;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn apex_pages_body() -> serde_json::Value {
        json!({"typeStubs": [{
            "name": "ApexPages",
            "namespacePrefix": "System",
            "kind": "CLASS",
            "modifiers": ["global"],
            "annotations": [],
            "superClass": null,
            "interfaces": [],
            "fields": [],
            "properties": [],
            "methods": [{
                "name": "addMessage",
                "isConstructor": null,
                "returnType": {"namespacePrefix": null, "name": "void", "typeParameters": null},
                "modifiers": ["global", "static"],
                "annotations": [],
                "parameters": [{
                    "name": "message",
                    "type": {"namespacePrefix": "ApexPages", "name": "Message", "typeParameters": null},
                    "annotations": [],
                    "documentation": ""
                }],
                "documentation": "Add a message to the current page context."
            }],
            "innerTypes": [],
            "triggerOperations": null,
            "documentation": null,
            "triggerObjectType": null,
            "compileError": null
        }]})
    }

    #[test]
    fn test_category_wire_values() {
        assert_eq!(SymbolCategory::Builtin.as_str(), "builtin");
        assert_eq!(SymbolCategory::Database.as_str(), "database");
        assert_eq!(SymbolCategory::Dynamic.as_str(), "dynamic");
    }

    #[test]
    fn test_query_params_keep_empty_namespace() {
        let req = SymbolsRequest::new(SymbolCategory::Database).namespace("");
        assert_eq!(
            req.query_params(),
            vec![("category", "database"), ("namespace", "")]
        );
    }

    #[test]
    fn test_decode_generic_type_ref() {
        let r: TypeRef = serde_json::from_value(json!({
            "namespacePrefix": "System", "name": "List",
            "typeParameters": [{"namespacePrefix": null, "name": "Account", "typeParameters": null}]
        }))
        .must();
        let params = r.type_parameters.must();
        assert_eq!(params[0].name, "Account");
    }

    #[tokio::test]
    async fn test_symbols_sends_filters_and_decodes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/v68.0/tooling/symbols/"))
            .and(query_param("category", "builtin"))
            .and(query_param("namespace", "System"))
            .and(query_param("name", "ApexPages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(apex_pages_body()))
            .expect(1)
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let req = SymbolsRequest::new(SymbolCategory::Builtin)
            .namespace("System")
            .name("ApexPages");
        let out = client.tooling().symbols(&req).await.must();
        let stub = &out.type_stubs[0];
        assert_eq!(stub.kind, "CLASS");
        assert_eq!(stub.methods[0].name, "addMessage");
        assert_eq!(
            stub.methods[0].parameters[0]
                .param_type
                .as_ref()
                .map(|t| t.name.as_str()),
            Some("Message")
        );
    }

    #[tokio::test]
    async fn test_symbols_omits_unset_filters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/services/data/v68.0/tooling/symbols/"))
            .and(query_param("category", "dynamic"))
            .and(query_param_is_missing("namespace"))
            .and(query_param_is_missing("name"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"typeStubs": []})))
            .expect(1)
            .mount(&server)
            .await;
        let client = builder()
            .authenticate(MockAuthenticator::new("t", &server.uri()))
            .build()
            .await
            .must();

        let out = client
            .tooling()
            .symbols(&SymbolsRequest::new(SymbolCategory::Dynamic))
            .await
            .must();
        assert!(out.type_stubs.is_empty());
    }
}
