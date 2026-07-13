//! Event-based parsing of SOAP Partner API response bodies.
//!
//! Responses are parsed into a small, namespace-prefix-agnostic DOM ([`Node`],
//! keyed on local element names) which the per-call parsers then navigate. This
//! keeps the parsers tolerant of prefix and element-ordering differences
//! between the canonical Partner WSDL shape and what a live org emits.

use super::types::{
    DeleteResult, DescribeGlobalResult, DescribeGlobalSObject, DescribeSObjectResult,
    FieldDescribe, PicklistEntry, QueryResult, SObject, SaveResult, SearchResult, SoapError,
    UpsertResult, UserInfo,
};
use crate::error::{ForceError, Result};
use quick_xml::Reader;
use quick_xml::events::{BytesRef, BytesStart, Event};

/// A minimal parsed XML element, keyed on its local (prefix-stripped) name.
#[derive(Debug, Default)]
pub struct Node {
    /// The local element name (namespace prefix stripped).
    pub name: String,
    /// The accumulated direct text content (entity references resolved).
    pub text: String,
    /// Whether the element carried `xsi:nil="true"`.
    pub nil: bool,
    /// Direct child elements, in document order.
    pub children: Vec<Self>,
}

impl Node {
    /// Returns the trimmed, owned text content of this node.
    pub fn text_owned(&self) -> String {
        self.text.trim().to_string()
    }

    /// Returns the first direct child element with the given local name.
    pub fn find_child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Returns the trimmed text of the first direct child with the given name.
    pub fn child_text(&self, name: &str) -> Option<String> {
        self.find_child(name).map(Self::text_owned)
    }

    /// Parses the first direct child with the given name as a boolean.
    fn child_bool(&self, name: &str) -> Option<bool> {
        self.child_text(name).and_then(|t| match t.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        })
    }

    /// Parses the first direct child with the given name as an `i64`.
    fn child_i64(&self, name: &str) -> Option<i64> {
        self.child_text(name).and_then(|t| t.parse().ok())
    }

    /// Iterates over direct children with the given local name.
    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Depth-first search for the first descendant (or self) with the given name.
    pub fn find_descendant(&self, name: &str) -> Option<&Self> {
        if self.name == name {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_descendant(name) {
                return Some(found);
            }
        }
        None
    }
}

/// Builds a node from a start/empty tag, capturing its local name and `xsi:nil`.
fn node_from_start(start: &BytesStart<'_>) -> Node {
    let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
    let mut nil = false;
    for attr in start.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"nil" && attr.value.as_ref() == b"true" {
            nil = true;
        }
    }
    Node {
        name,
        nil,
        ..Node::default()
    }
}

/// Resolves an XML general/character entity reference to its text.
fn resolve_ref(reference: &BytesRef<'_>) -> String {
    if let Ok(Some(ch)) = reference.resolve_char_ref() {
        return ch.to_string();
    }
    if let Ok(name) = reference.decode() {
        if let Some(resolved) = quick_xml::escape::resolve_predefined_entity(&name) {
            return resolved.to_string();
        }
    }
    String::new()
}

/// Parses an XML document into a synthetic root [`Node`] whose children are the
/// document's top-level elements.
pub fn parse_document(xml: &str) -> Result<Node> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => stack.push(node_from_start(&start)),
            Ok(Event::Empty(start)) => {
                let node = node_from_start(&start);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Ok(Event::Text(text)) => {
                if let (Some(parent), Ok(decoded)) = (stack.last_mut(), text.decode()) {
                    parent.text.push_str(&decoded);
                }
            }
            Ok(Event::CData(cdata)) => {
                if let (Some(parent), Ok(decoded)) = (stack.last_mut(), cdata.decode()) {
                    parent.text.push_str(&decoded);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(parent) = stack.last_mut() {
                    parent.text.push_str(&resolve_ref(&reference));
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    if let Some(node) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ForceError::InvalidInput(format!(
                    "failed to parse SOAP response XML: {err}"
                )));
            }
            Ok(_) => {}
        }
    }

    stack
        .into_iter()
        .next()
        .ok_or_else(|| ForceError::InvalidInput("empty SOAP response".to_string()))
}

/// Returns the `<result>` element nodes beneath the operation response element.
fn operation_result_nodes(root: &Node) -> Vec<&Node> {
    let Some(body) = root.find_descendant("Body") else {
        return Vec::new();
    };
    // The single element child of <Body> is the `<...Response>` wrapper.
    let Some(response) = body.children.iter().find(|c| !c.name.is_empty()) else {
        return Vec::new();
    };
    response.children_named("result").collect()
}

/// Parses a per-record `<errors>` element.
fn parse_error(node: &Node) -> SoapError {
    SoapError {
        status_code: node.child_text("statusCode").unwrap_or_default(),
        message: node.child_text("message").unwrap_or_default(),
        fields: node
            .children_named("fields")
            .map(Node::text_owned)
            .collect(),
    }
}

/// Parses a response `sObject` (a `<records>` or `<record>` element).
///
/// De-duplicates the repeated `Id` element (a Partner WSDL quirk) and skips
/// nested relationship records/subqueries, which are a documented follow-up.
fn parse_record(node: &Node) -> SObject {
    let mut obj = SObject::new(String::new());
    let mut seen_id = false;
    for child in &node.children {
        if child.name == "type" {
            obj.sobject_type = child.text_owned();
            continue;
        }
        if child.name == "Id" {
            if seen_id {
                continue;
            }
            seen_id = true;
        }
        // Nested relationship records / child subqueries carry element children;
        // skip them for the v1 flat model.
        if child.children.iter().any(|c| !c.name.is_empty()) {
            continue;
        }
        let value = if child.nil {
            None
        } else {
            Some(child.text_owned())
        };
        obj.fields.push((child.name.clone(), value));
    }
    obj
}

/// Parses a `createResponse`/`updateResponse` body into `SaveResult`s.
pub fn parse_save_results(xml: &str) -> Result<Vec<SaveResult>> {
    let root = parse_document(xml)?;
    Ok(operation_result_nodes(&root)
        .into_iter()
        .map(|node| SaveResult {
            id: node.child_text("id").filter(|s| !s.is_empty()),
            success: node.child_bool("success").unwrap_or(false),
            errors: node.children_named("errors").map(parse_error).collect(),
        })
        .collect())
}

/// Parses an `upsertResponse` body into `UpsertResult`s.
pub fn parse_upsert_results(xml: &str) -> Result<Vec<UpsertResult>> {
    let root = parse_document(xml)?;
    Ok(operation_result_nodes(&root)
        .into_iter()
        .map(|node| UpsertResult {
            created: node.child_bool("created").unwrap_or(false),
            id: node.child_text("id").filter(|s| !s.is_empty()),
            success: node.child_bool("success").unwrap_or(false),
            errors: node.children_named("errors").map(parse_error).collect(),
        })
        .collect())
}

/// Parses a `deleteResponse` body into `DeleteResult`s.
pub fn parse_delete_results(xml: &str) -> Result<Vec<DeleteResult>> {
    let root = parse_document(xml)?;
    Ok(operation_result_nodes(&root)
        .into_iter()
        .map(|node| DeleteResult {
            id: node.child_text("id").filter(|s| !s.is_empty()),
            success: node.child_bool("success").unwrap_or(false),
            errors: node.children_named("errors").map(parse_error).collect(),
        })
        .collect())
}

/// Parses a `retrieveResponse` body into records, skipping nil (not-found) slots.
pub fn parse_retrieve(xml: &str) -> Result<Vec<SObject>> {
    let root = parse_document(xml)?;
    Ok(operation_result_nodes(&root)
        .into_iter()
        .filter(|node| !node.nil)
        .map(parse_record)
        .collect())
}

/// Parses a `query`/`queryMore`/`queryAll` response body into a [`QueryResult`].
pub fn parse_query_result(xml: &str) -> Result<QueryResult> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    let Some(result) = results.into_iter().next() else {
        return Ok(QueryResult {
            done: true,
            query_locator: None,
            size: 0,
            records: Vec::new(),
        });
    };
    Ok(QueryResult {
        done: result.child_bool("done").unwrap_or(true),
        query_locator: result.child_text("queryLocator").filter(|s| !s.is_empty()),
        size: result.child_i64("size").unwrap_or(0),
        records: result.children_named("records").map(parse_record).collect(),
    })
}

/// Parses a `searchResponse` body into a [`SearchResult`].
pub fn parse_search_result(xml: &str) -> Result<SearchResult> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    let Some(result) = results.into_iter().next() else {
        return Ok(SearchResult::default());
    };
    let records = result
        .children_named("searchRecords")
        .map(|sr| {
            sr.find_child("record")
                .map_or_else(|| parse_record(sr), parse_record)
        })
        .collect();
    Ok(SearchResult { records })
}

/// Parses a picklist entry element.
fn parse_picklist_entry(node: &Node) -> PicklistEntry {
    PicklistEntry {
        value: node.child_text("value").unwrap_or_default(),
        label: node.child_text("label"),
        active: node.child_bool("active"),
        default_value: node.child_bool("defaultValue"),
    }
}

/// Parses a `<fields>` element from a describe result.
fn parse_field_describe(node: &Node) -> FieldDescribe {
    FieldDescribe {
        name: node.child_text("name").unwrap_or_default(),
        label: node.child_text("label"),
        field_type: node.child_text("type"),
        length: node.child_i64("length"),
        nillable: node.child_bool("nillable"),
        custom: node.child_bool("custom"),
        picklist_values: node
            .children_named("picklistValues")
            .map(parse_picklist_entry)
            .collect(),
    }
}

/// Parses a `describeSObjectResponse` body into a [`DescribeSObjectResult`].
pub fn parse_describe_sobject(xml: &str) -> Result<DescribeSObjectResult> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    let result = results
        .into_iter()
        .next()
        .ok_or_else(|| ForceError::InvalidInput("missing describeSObject result".to_string()))?;
    Ok(describe_sobject_from_node(result))
}

/// Parses a `describeSObjectsResponse` body into multiple describe results.
pub fn parse_describe_sobjects(xml: &str) -> Result<Vec<DescribeSObjectResult>> {
    let root = parse_document(xml)?;
    Ok(operation_result_nodes(&root)
        .into_iter()
        .map(describe_sobject_from_node)
        .collect())
}

/// Builds a [`DescribeSObjectResult`] from a `<result>` node.
fn describe_sobject_from_node(result: &Node) -> DescribeSObjectResult {
    DescribeSObjectResult {
        name: result.child_text("name").unwrap_or_default(),
        label: result.child_text("label"),
        label_plural: result.child_text("labelPlural"),
        key_prefix: result.child_text("keyPrefix").filter(|s| !s.is_empty()),
        custom: result.child_bool("custom").unwrap_or(false),
        createable: result.child_bool("createable").unwrap_or(false),
        updateable: result.child_bool("updateable").unwrap_or(false),
        deletable: result.child_bool("deletable").unwrap_or(false),
        queryable: result.child_bool("queryable").unwrap_or(false),
        fields: result
            .children_named("fields")
            .map(parse_field_describe)
            .collect(),
    }
}

/// Parses a `describeGlobalResponse` body into a [`DescribeGlobalResult`].
pub fn parse_describe_global(xml: &str) -> Result<DescribeGlobalResult> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    let Some(result) = results.into_iter().next() else {
        return Ok(DescribeGlobalResult::default());
    };
    Ok(DescribeGlobalResult {
        encoding: result.child_text("encoding"),
        max_batch_size: result.child_i64("maxBatchSize"),
        sobjects: result
            .children_named("sobjects")
            .map(|node| DescribeGlobalSObject {
                name: node.child_text("name").unwrap_or_default(),
                label: node.child_text("label"),
                key_prefix: node.child_text("keyPrefix").filter(|s| !s.is_empty()),
                custom: node.child_bool("custom").unwrap_or(false),
                createable: node.child_bool("createable").unwrap_or(false),
                queryable: node.child_bool("queryable").unwrap_or(false),
                updateable: node.child_bool("updateable").unwrap_or(false),
                deletable: node.child_bool("deletable").unwrap_or(false),
            })
            .collect(),
    })
}

/// Parses a `getUserInfoResponse` body into a [`UserInfo`].
pub fn parse_user_info(xml: &str) -> Result<UserInfo> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    let result = results
        .into_iter()
        .next()
        .ok_or_else(|| ForceError::InvalidInput("missing getUserInfo result".to_string()))?;
    Ok(UserInfo {
        user_id: result.child_text("userId").unwrap_or_default(),
        user_full_name: result.child_text("userFullName").unwrap_or_default(),
        user_email: result.child_text("userEmail").unwrap_or_default(),
        user_name: result.child_text("userName").unwrap_or_default(),
        organization_id: result.child_text("organizationId").unwrap_or_default(),
        organization_name: result.child_text("organizationName").unwrap_or_default(),
        profile_id: result.child_text("profileId").filter(|s| !s.is_empty()),
        role_id: result.child_text("roleId").filter(|s| !s.is_empty()),
        session_seconds_valid: result.child_i64("sessionSecondsValid"),
        user_default_currency_iso_code: result.child_text("userDefaultCurrencyIsoCode"),
        user_language: result.child_text("userLanguage"),
        user_locale: result.child_text("userLocale"),
        user_time_zone: result.child_text("userTimeZone"),
        user_type: result.child_text("userType"),
        currency_symbol: result.child_text("currencySymbol"),
        org_default_currency_iso_code: result.child_text("orgDefaultCurrencyIsoCode"),
        org_disallow_html_attachments: result.child_bool("orgDisallowHtmlAttachments"),
        org_has_person_accounts: result.child_bool("orgHasPersonAccounts"),
        accessibility_mode: result.child_bool("accessibilityMode"),
    })
}

/// Parses a `getServerTimestampResponse` body into its ISO-8601 timestamp string.
pub fn parse_server_timestamp(xml: &str) -> Result<String> {
    let root = parse_document(xml)?;
    let results = operation_result_nodes(&root);
    results
        .into_iter()
        .next()
        .and_then(|result| result.child_text("timestamp"))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ForceError::InvalidInput("missing server timestamp".to_string()))
}
