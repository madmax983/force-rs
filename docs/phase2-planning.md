# Phase 2 Planning: REST API Implementation

**Status:** Planning
**Start Date:** 2026-02-07
**Dependencies:** Phase 1 complete (ForceClient, TokenManager, error hierarchy)
**Owner:** docs-specialist, types-specialist

---

## Overview

Phase 2 implements the Salesforce REST API surface, the default and most commonly used API. This phase validates the handler pattern (ADR-006) and establishes patterns for all future API surfaces.

**Goals:**
1. Implement `RestHandler` providing REST API operations
2. Support SOQL queries with typed responses and pagination
3. Implement CRUD operations (create, read, update, delete, upsert)
4. Provide describe API for metadata and schema discovery
5. Establish Salesforce ↔ Rust type mapping conventions
6. Target 85-90% test coverage with TDD discipline

---

## 1. Handler Architecture

### Validating ADR-006: Handler Pattern

The `RestHandler` validates our decision to use handler objects for API organization.

#### Handler Design

```rust
// crates/force/src/api/rest/mod.rs

/// REST API handler providing SOQL, CRUD, describe, and search operations.
///
/// Access via `client.rest()`.
///
/// # Examples
///
/// ```
/// let client = ForceClient::builder()
///     .with_client_credentials("id", "secret")
///     .build()?;
///
/// // Query
/// let accounts = client.rest()
///     .query::<Account>("SELECT Id, Name FROM Account LIMIT 5")
///     .await?;
///
/// // Create
/// let contact_id = client.rest()
///     .create("Contact", &contact_data)
///     .await?;
/// ```
pub struct RestHandler<'a, Auth> {
    client: &'a ForceClient<Auth>,
}

impl<'a, Auth: Authenticator> RestHandler<'a, Auth> {
    pub(crate) fn new(client: &'a ForceClient<Auth>) -> Self {
        Self { client }
    }

    // === Query Operations ===

    /// Execute a SOQL query with typed response deserialization.
    pub async fn query<T: DeserializeOwned>(
        &self,
        soql: &str,
    ) -> Result<QueryResult<T>> {
        // Implementation
    }

    /// Execute a SOQL query with automatic pagination.
    pub async fn query_all<T: DeserializeOwned>(
        &self,
        soql: &str,
    ) -> Result<impl Stream<Item = Result<T>>> {
        // Returns a stream that automatically follows nextRecordsUrl
    }

    /// Continue a query using a query locator.
    pub async fn query_more<T: DeserializeOwned>(
        &self,
        query_locator: &str,
    ) -> Result<QueryResult<T>> {
        // Implementation
    }

    // === CRUD Operations ===

    /// Create a new record.
    pub async fn create(
        &self,
        sobject: &str,
        data: &serde_json::Value,
    ) -> Result<CreateResponse> {
        // POST /sobjects/{sobject}
    }

    /// Get a record by ID.
    pub async fn get(
        &self,
        sobject: &str,
        id: &SalesforceId,
    ) -> Result<serde_json::Value> {
        // GET /sobjects/{sobject}/{id}
    }

    /// Get specific fields from a record.
    pub async fn get_fields(
        &self,
        sobject: &str,
        id: &SalesforceId,
        fields: &[&str],
    ) -> Result<serde_json::Value> {
        // GET /sobjects/{sobject}/{id}?fields=Field1,Field2
    }

    /// Update a record.
    pub async fn update(
        &self,
        sobject: &str,
        id: &SalesforceId,
        data: &serde_json::Value,
    ) -> Result<()> {
        // PATCH /sobjects/{sobject}/{id}
    }

    /// Delete a record.
    pub async fn delete(
        &self,
        sobject: &str,
        id: &SalesforceId,
    ) -> Result<()> {
        // DELETE /sobjects/{sobject}/{id}
    }

    /// Upsert a record (insert or update based on external ID).
    pub async fn upsert(
        &self,
        sobject: &str,
        external_id_field: &str,
        external_id_value: &str,
        data: &serde_json::Value,
    ) -> Result<UpsertResponse> {
        // PATCH /sobjects/{sobject}/{field}/{value}
    }

    // === Search Operations ===

    /// Execute a SOSL search query.
    pub async fn search(
        &self,
        sosl: &str,
    ) -> Result<SearchResult> {
        // GET /search?q=FIND+{query}
    }

    // === Describe Operations ===

    /// Describe global (list all objects).
    pub async fn describe_global(&self) -> Result<DescribeGlobalResult> {
        // GET /sobjects/
    }

    /// Describe a specific SObject.
    pub async fn describe(
        &self,
        sobject: &str,
    ) -> Result<DescribeSObjectResult> {
        // GET /sobjects/{sobject}/describe
    }

    /// Get basic SObject metadata without full describe.
    pub async fn basic_info(
        &self,
        sobject: &str,
    ) -> Result<SObjectBasicInfo> {
        // GET /sobjects/{sobject}
    }

    // === Limits ===

    /// Get org limits.
    pub async fn limits(&self) -> Result<OrganizationLimits> {
        // GET /limits
    }
}
```

#### Module Structure

```
crates/force/src/api/
├── mod.rs                    # Re-exports
└── rest/
    ├── mod.rs                # RestHandler
    ├── query.rs              # Query types (QueryResult, Stream)
    ├── crud.rs               # CRUD types (CreateResponse, UpsertResponse)
    ├── describe.rs           # Describe types (DescribeGlobalResult, etc.)
    ├── search.rs             # SOSL types (SearchResult)
    └── limits.rs             # Limits types (OrganizationLimits)
```

---

## 2. SOQL Query Design

### Typed Queries with Generics

Users provide a type parameter for deserialization, allowing type-safe query results.

#### QueryResult Type

```rust
// crates/force/src/api/rest/query.rs

use serde::{Deserialize, Serialize};

/// Result of a SOQL query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult<T> {
    /// Total number of records (may be approximate for large result sets)
    pub total_size: u64,

    /// Whether all results have been retrieved
    pub done: bool,

    /// Query locator for fetching next batch (if done = false)
    pub next_records_url: Option<String>,

    /// Retrieved records
    pub records: Vec<T>,
}

impl<T> QueryResult<T> {
    /// Check if more records are available.
    pub fn has_more(&self) -> bool {
        !self.done && self.next_records_url.is_some()
    }

    /// Extract the query locator for pagination.
    pub fn query_locator(&self) -> Option<&str> {
        self.next_records_url
            .as_ref()
            .and_then(|url| url.split('/').last())
    }
}
```

#### Usage Patterns

**Dynamic Queries (serde_json::Value):**
```rust
// Untyped, flexible
let result = client.rest()
    .query::<serde_json::Value>("SELECT Id, Name FROM Account")
    .await?;

for record in result.records {
    println!("{}: {}", record["Id"], record["Name"]);
}
```

**Typed Queries:**
```rust
#[derive(Debug, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: SalesforceId,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

let result = client.rest()
    .query::<Account>("SELECT Id, Name, Industry FROM Account")
    .await?;

for account in result.records {
    println!("{}: {} ({})", account.id, account.name,
        account.industry.as_deref().unwrap_or("N/A"));
}
```

**Manual Pagination:**
```rust
let mut result = client.rest()
    .query::<Account>("SELECT Id, Name FROM Account")
    .await?;

let mut all_accounts = result.records;

while result.has_more() {
    let locator = result.query_locator().unwrap();
    result = client.rest().query_more::<Account>(locator).await?;
    all_accounts.extend(result.records);
}

println!("Total accounts: {}", all_accounts.len());
```

**Automatic Pagination with Streams:**
```rust
use futures::StreamExt;

let mut stream = client.rest()
    .query_all::<Account>("SELECT Id, Name FROM Account")
    .await?;

while let Some(account) = stream.next().await {
    let account = account?;
    println!("{}: {}", account.id, account.name);
}
```

### Stream Implementation

```rust
// crates/force/src/api/rest/query.rs

use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Stream of SOQL query results with automatic pagination.
pub struct QueryStream<'a, Auth, T> {
    handler: &'a RestHandler<'a, Auth>,
    buffer: Vec<T>,
    next_locator: Option<String>,
    done: bool,
}

impl<'a, Auth: Authenticator, T: DeserializeOwned> Stream for QueryStream<'a, Auth, T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Pop from buffer if available
        if let Some(record) = self.buffer.pop() {
            return Poll::Ready(Some(Ok(record)));
        }

        // If done, no more records
        if self.done {
            return Poll::Ready(None);
        }

        // Fetch next batch
        // ... async fetch logic ...
        Poll::Pending
    }
}
```

---

## 3. Describe API

### Metadata Type Design

```rust
// crates/force/src/api/rest/describe.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Result of describe global (all objects in org).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeGlobalResult {
    /// API encoding (usually UTF-8)
    pub encoding: String,

    /// Max batch size
    pub max_batch_size: u32,

    /// All SObjects in the org
    pub sobjects: Vec<SObjectBasicInfo>,
}

/// Basic SObject information (from describe global).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SObjectBasicInfo {
    /// Object name (e.g., "Account")
    pub name: String,

    /// Object label (e.g., "Account")
    pub label: String,

    /// Custom object?
    pub custom: bool,

    /// API name with namespace prefix if applicable
    pub key_prefix: Option<String>,

    /// Label in plural form
    pub label_plural: String,

    /// Layoutable?
    pub layoutable: bool,

    /// Activateable?
    pub activateable: bool,

    /// Whether this object can be queried
    pub queryable: bool,

    /// Recent items in the object
    pub urls: SObjectUrls,
}

/// Full SObject metadata (from describe).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeSObjectResult {
    /// Object name
    pub name: String,

    /// Object label
    pub label: String,

    /// Custom object?
    pub custom: bool,

    /// Fields
    pub fields: Vec<FieldDescribe>,

    /// Record type infos
    pub record_type_infos: Vec<RecordTypeInfo>,

    /// Child relationships
    pub child_relationships: Vec<ChildRelationship>,

    /// Whether this object is createable
    pub createable: bool,

    /// Whether this object is updateable
    pub updateable: bool,

    /// Whether this object is deleteable
    pub deleteable: bool,

    /// Whether this object is queryable
    pub queryable: bool,

    /// Whether this object is searchable
    pub searchable: bool,

    /// URLs for accessing this object
    pub urls: SObjectUrls,
}

/// Field metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDescribe {
    /// Field API name
    pub name: String,

    /// Field label
    pub label: String,

    /// Field type (string, double, boolean, date, datetime, etc.)
    #[serde(rename = "type")]
    pub field_type: String,

    /// Length (for strings)
    pub length: Option<u32>,

    /// Precision (for numbers)
    pub precision: Option<u32>,

    /// Scale (for decimals)
    pub scale: Option<u32>,

    /// Whether field is custom
    pub custom: bool,

    /// Whether field can be null
    pub nillable: bool,

    /// Whether field is createable
    pub createable: bool,

    /// Whether field is updateable
    pub updateable: bool,

    /// Picklist values (if applicable)
    pub picklist_values: Option<Vec<PicklistValue>>,

    /// Reference to (for relationship fields)
    pub reference_to: Option<Vec<String>>,

    /// Relationship name (for relationship fields)
    pub relationship_name: Option<String>,

    /// Whether this is an ID field
    pub id_lookup: bool,

    /// Whether this is an external ID
    pub external_id: bool,

    /// Whether this field is unique
    pub unique: bool,
}

/// Picklist value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicklistValue {
    pub label: String,
    pub value: String,
    pub default_value: bool,
    pub active: bool,
}

/// Record type information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordTypeInfo {
    pub name: String,
    pub record_type_id: Option<SalesforceId>,
    pub available: bool,
    pub default_record_type_mapping: bool,
    pub master: bool,
}

/// Child relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildRelationship {
    pub child_sobject: String,
    pub field: String,
    pub relationship_name: Option<String>,
    pub cascade_delete: bool,
    pub restricted_delete: bool,
}

/// SObject URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SObjectUrls {
    pub sobject: String,
    pub describe: Option<String>,
    pub row_template: Option<String>,
}
```

### Schema Caching Strategy

**Option 1: No Caching (Phase 2 MVP)**
- Simple: no cache, describe on every call
- Suitable for low-frequency describe operations

**Option 2: In-Memory Cache (Future)**
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct SchemaCache {
    cache: Arc<RwLock<HashMap<String, DescribeSObjectResult>>>,
    ttl: Duration,
}

impl SchemaCache {
    pub async fn get_or_fetch(
        &self,
        sobject: &str,
        fetcher: impl Future<Output = Result<DescribeSObjectResult>>,
    ) -> Result<DescribeSObjectResult> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(result) = cache.get(sobject) {
                return Ok(result.clone());
            }
        }

        // Fetch and cache
        let result = fetcher.await?;
        {
            let mut cache = self.cache.write().await;
            cache.insert(sobject.to_string(), result.clone());
        }
        Ok(result)
    }
}
```

**Recommendation:** Start without caching, add in Phase 3 if needed.

---

## 4. Type Mapping: Salesforce ↔ Rust

### Core Type Conversions

| Salesforce Type | Rust Type | Notes |
|-----------------|-----------|-------|
| `id` | `SalesforceId` | 15 or 18 char validated |
| `string` | `String` | |
| `int` | `i32` | |
| `long` | `i64` | |
| `double` | `f64` | |
| `currency` | `rust_decimal::Decimal` | Requires `decimal` feature |
| `percent` | `f64` | |
| `boolean` | `bool` | |
| `date` | `chrono::NaiveDate` | YYYY-MM-DD |
| `datetime` | `chrono::DateTime<Utc>` | ISO 8601 |
| `time` | `chrono::NaiveTime` | HH:MM:SS.mmm |
| `picklist` | `String` or `enum` | User's choice |
| `multipicklist` | `Vec<String>` | Semicolon-separated |
| `reference` | `SalesforceId` | Foreign key |
| `email` | `String` | |
| `phone` | `String` | |
| `url` | `String` or `url::Url` | |
| `textarea` | `String` | |
| `address` | Custom struct | Compound field |
| `location` | Custom struct | Lat/long |

### Date and DateTime Handling

**Using chrono:**
```rust
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Event {
    #[serde(rename = "Id")]
    id: SalesforceId,

    #[serde(rename = "Subject")]
    subject: String,

    /// Date field (no time component)
    #[serde(rename = "ActivityDate")]
    activity_date: NaiveDate,  // "2024-01-15"

    /// DateTime field (with timezone, always UTC)
    #[serde(rename = "CreatedDate")]
    created_date: DateTime<Utc>,  // "2024-01-15T10:30:00.000+0000"

    /// Time field (no date component)
    #[serde(rename = "StartTime")]
    start_time: Option<NaiveTime>,  // "10:30:00.000"
}
```

**Serialization Examples:**
```rust
// Date: YYYY-MM-DD
let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
assert_eq!(serde_json::to_string(&date)?, "\"2024-01-15\"");

// DateTime: ISO 8601 with UTC
let dt = Utc::now();
// Serializes to "2024-01-15T10:30:00.000Z"
```

### Decimal Handling

**Option 1: f64 (Simple, Lossy)**
```rust
#[derive(Debug, Deserialize)]
struct Product {
    #[serde(rename = "UnitPrice")]
    unit_price: f64,  // ⚠️ Potential precision loss
}
```

**Option 2: rust_decimal::Decimal (Precise, Feature-Gated)**
```toml
# Cargo.toml
[dependencies]
rust_decimal = { version = "1.34", features = ["serde"], optional = true }

[features]
decimal = ["dep:rust_decimal"]
```

```rust
#[cfg(feature = "decimal")]
use rust_decimal::Decimal;

#[derive(Debug, Deserialize)]
struct Product {
    #[serde(rename = "UnitPrice")]
    #[cfg(feature = "decimal")]
    unit_price: Decimal,  // ✅ Exact decimal arithmetic

    #[cfg(not(feature = "decimal"))]
    unit_price: f64,
}
```

**Recommendation:** Provide both options, default to f64, document precision trade-offs.

### Compound Fields (Address, Location)

**Address:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

// In Account:
#[derive(Debug, Deserialize)]
struct Account {
    #[serde(flatten, with = "address_prefix")]
    billing_address: Address,
    // Salesforce fields: BillingStreet, BillingCity, etc.
}

// Custom deserializer module
mod address_prefix {
    // Handles BillingStreet -> Address.street mapping
}
```

**Location:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Deserialize)]
struct Store {
    #[serde(rename = "Location__c")]
    location: GeoLocation,  // Compound field
}
```

### Picklist as Enum

**String (Simple):**
```rust
#[derive(Debug, Deserialize)]
struct Lead {
    #[serde(rename = "Status")]
    status: String,  // "Open", "Qualified", "Unqualified"
}
```

**Enum (Type-Safe):**
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum LeadStatus {
    Open,
    Contacted,
    Qualified,
    Unqualified,
}

#[derive(Debug, Deserialize)]
struct Lead {
    #[serde(rename = "Status")]
    status: LeadStatus,
}
```

**Recommendation:** Document both patterns, let users choose based on their needs.

---

## 5. CRUD Operations

### Create

```rust
// crates/force/src/api/rest/crud.rs

/// Response from creating a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponse {
    /// ID of the created record
    pub id: SalesforceId,

    /// Whether the operation succeeded
    pub success: bool,

    /// Errors (if any)
    pub errors: Vec<ApiError>,
}
```

**Usage:**
```rust
let contact = serde_json::json!({
    "FirstName": "John",
    "LastName": "Doe",
    "Email": "john.doe@example.com",
    "AccountId": account_id.as_str(),
});

let response = client.rest()
    .create("Contact", &contact)
    .await?;

println!("Created contact: {}", response.id);
```

### Read (Get)

```rust
let account = client.rest()
    .get("Account", &account_id)
    .await?;

println!("Account name: {}", account["Name"]);

// Or with specific fields:
let fields = vec!["Id", "Name", "Industry"];
let account = client.rest()
    .get_fields("Account", &account_id, &fields)
    .await?;
```

### Update

```rust
let updates = serde_json::json!({
    "Phone": "555-0100",
    "Industry": "Technology",
});

client.rest()
    .update("Account", &account_id, &updates)
    .await?;
```

### Delete

```rust
client.rest()
    .delete("Account", &account_id)
    .await?;
```

### Upsert

```rust
/// Response from upserting a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertResponse {
    /// ID of the upserted record
    pub id: SalesforceId,

    /// Whether this was a create (true) or update (false)
    pub created: bool,

    /// Whether the operation succeeded
    pub success: bool,

    /// Errors (if any)
    pub errors: Vec<ApiError>,
}
```

**Usage:**
```rust
let account = serde_json::json!({
    "Name": "Acme Corp",
    "Industry": "Technology",
});

// Upsert by external ID
let response = client.rest()
    .upsert("Account", "ExternalId__c", "ACME-001", &account)
    .await?;

if response.created {
    println!("Created new account: {}", response.id);
} else {
    println!("Updated existing account: {}", response.id);
}
```

---

## 6. Organization Limits

```rust
// crates/force/src/api/rest/limits.rs

use serde::{Deserialize, Serialize};

/// Organization limits and usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OrganizationLimits {
    pub daily_api_requests: LimitInfo,
    pub daily_async_apex_executions: LimitInfo,
    pub daily_bulk_api_requests: LimitInfo,
    pub daily_streaming_api_events: LimitInfo,
    pub hourly_o_data_call_out: LimitInfo,
    // ... many more limits
}

/// Limit information (max and remaining).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LimitInfo {
    pub max: u64,
    pub remaining: u64,
}

impl LimitInfo {
    /// Percentage used (0.0 to 1.0).
    pub fn usage_percent(&self) -> f64 {
        if self.max == 0 {
            0.0
        } else {
            1.0 - (self.remaining as f64 / self.max as f64)
        }
    }

    /// Check if nearing limit (>= 80% used).
    pub fn is_near_limit(&self) -> bool {
        self.usage_percent() >= 0.8
    }
}
```

**Usage:**
```rust
let limits = client.rest().limits().await?;

println!("API calls: {} / {}",
    limits.daily_api_requests.max - limits.daily_api_requests.remaining,
    limits.daily_api_requests.max
);

if limits.daily_api_requests.is_near_limit() {
    eprintln!("⚠️  API limit nearly exhausted!");
}
```

---

## 7. Testing Strategy

### Unit Tests (Per Module)

```rust
// crates/force/src/api/rest/query.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_result_has_more() {
        let result = QueryResult {
            total_size: 100,
            done: false,
            next_records_url: Some("/query/locator".to_string()),
            records: vec![],
        };

        assert!(result.has_more());
    }

    #[test]
    fn test_query_locator_extraction() {
        let result = QueryResult::<()> {
            total_size: 100,
            done: false,
            next_records_url: Some("/services/data/v60.0/query/01gxx0000000001-2000".to_string()),
            records: vec![],
        };

        assert_eq!(result.query_locator(), Some("01gxx0000000001-2000"));
    }
}
```

### Integration Tests

```rust
// tests/rest_api.rs

use force::{ForceClient, RestHandler};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_query_account() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/services/data/v60.0/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{
                "attributes": {"type": "Account"},
                "Id": "001000000000001AAA",
                "Name": "Acme Corp"
            }]
        })))
        .mount(&mock_server)
        .await;

    let client = ForceClient::builder()
        .instance_url(mock_server.uri())
        .with_mock_auth()  // Mock authenticator
        .build()?;

    let result = client.rest()
        .query::<serde_json::Value>("SELECT Id, Name FROM Account")
        .await?;

    assert_eq!(result.total_size, 1);
    assert_eq!(result.records[0]["Name"], "Acme Corp");
}
```

---

## 8. Error Handling

All REST operations use the error hierarchy from ADR-003:

```rust
match client.rest().query::<Account>("SELECT Id FROM Account").await {
    Ok(result) => { /* ... */ }
    Err(ForceError::Auth(auth_err)) => {
        // Handle authentication errors
    }
    Err(ForceError::Http(http_err)) => {
        // Handle HTTP errors (network, timeout, 4xx, 5xx)
    }
    Err(ForceError::Api(api_err)) => {
        // Handle Salesforce API errors (SOQL syntax, field errors)
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

---

## 9. Implementation Phases

### Phase 2.1: Query Operations (Week 1)
- [ ] QueryResult<T> type
- [ ] RestHandler::query()
- [ ] RestHandler::query_more()
- [ ] Pagination logic
- [ ] Tests: basic queries, pagination, typed responses

### Phase 2.2: CRUD Operations (Week 1-2)
- [ ] CreateResponse, UpsertResponse types
- [ ] RestHandler::create()
- [ ] RestHandler::get()
- [ ] RestHandler::update()
- [ ] RestHandler::delete()
- [ ] RestHandler::upsert()
- [ ] Tests: create, read, update, delete, upsert

### Phase 2.3: Describe API (Week 2)
- [ ] Describe types (DescribeSObjectResult, FieldDescribe, etc.)
- [ ] RestHandler::describe_global()
- [ ] RestHandler::describe()
- [ ] RestHandler::basic_info()
- [ ] Tests: describe parsing, field metadata

### Phase 2.4: Search & Limits (Week 2)
- [ ] SearchResult types
- [ ] RestHandler::search()
- [ ] OrganizationLimits types
- [ ] RestHandler::limits()
- [ ] Tests: SOSL search, limits parsing

### Phase 2.5: Streams (Week 3)
- [ ] QueryStream implementation
- [ ] RestHandler::query_all()
- [ ] Automatic pagination
- [ ] Tests: stream iteration, error handling

### Phase 2.6: Polish (Week 3)
- [ ] Documentation examples
- [ ] Integration tests with mock server
- [ ] Performance testing
- [ ] Code coverage analysis (target: 85-90%)

---

## 10. Dependencies & Coordination

### Type Coordination (with types-specialist)

**New Types Needed:**
- `Address` - Compound address field
- `GeoLocation` - Latitude/longitude
- Common enums (LeadStatus, AccountType, etc.) - **Optional, user-defined**

**Existing Types to Use:**
- `SalesforceId` ✅
- `ApiVersion` ✅

**Feature Flags:**
- `decimal` - Adds rust_decimal support for currency fields

### Dependencies on Phase 1

All Phase 1 tasks must be complete:
- ✅ ForceClient with authentication
- ✅ TokenManager for token handling
- ✅ Error hierarchy
- ✅ HTTP client with middleware

---

## 11. Open Questions

### Q1: Should we provide a query builder?
**Option A:** String-based queries (Phase 2 MVP)
```rust
client.rest().query::<Account>("SELECT Id, Name FROM Account").await?;
```

**Option B:** Type-safe query builder (Future Phase)
```rust
client.rest()
    .query_builder::<Account>()
    .select(&["Id", "Name"])
    .where_clause("Industry = 'Technology'")
    .limit(10)
    .execute()
    .await?;
```

**Recommendation:** Start with string-based (Option A), evaluate builder in Phase 3.

### Q2: How should we handle large decimal values?
**Options:**
1. Default to `f64` (simple, lossy for very large values)
2. Default to `rust_decimal::Decimal` (precise, adds dependency)
3. Feature flag: `f64` without feature, `Decimal` with `decimal` feature

**Recommendation:** Option 3 (feature-gated).

### Q3: Should describe results be cached?
**Recommendation:** No caching in Phase 2 MVP. Add in Phase 3 if benchmarks show it's needed.

### Q4: Should we generate SObject structs?
**Options:**
1. Users define their own structs (Phase 2 approach)
2. Provide a codegen tool to generate from org metadata (Future)

**Recommendation:** Manual structs in Phase 2, codegen as separate crate later.

---

## 12. Success Metrics

Phase 2 is successful when:
- ✅ All CRUD operations implemented with TDD
- ✅ SOQL queries work with typed and dynamic responses
- ✅ Pagination works (manual and automatic)
- ✅ Describe API returns complete metadata
- ✅ 85-90% test coverage achieved
- ✅ Integration tests with wiremock pass
- ✅ Documentation examples are clear and runnable
- ✅ Zero clippy warnings with pedantic/nursery lints

---

## 13. Post-Phase 2

After Phase 2 completion, we can proceed to:
- **Phase 3:** Bulk API 2.0 (CSV upload/download, large data operations)
- **Phase 4:** Composite API (batch requests, transaction control)
- **Phase 5:** Tooling API (metadata deployment, debug logs)

---

## Appendix: Example Integration

**Complete Example:**
```rust
use force::{ForceClient, ClientConfig, Environment};
use serde::{Deserialize, Serialize};
use chrono::DateTime;

#[derive(Debug, Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: SalesforceId,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
    #[serde(rename = "CreatedDate")]
    created_date: DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Build client
    let client = ForceClient::builder()
        .environment(Environment::Production)
        .with_client_credentials(
            env::var("SF_CLIENT_ID")?,
            env::var("SF_CLIENT_SECRET")?
        )
        .build()?;

    // Query accounts
    let result = client.rest()
        .query::<Account>("SELECT Id, Name, Industry, CreatedDate FROM Account LIMIT 10")
        .await?;

    println!("Found {} accounts", result.total_size);

    for account in result.records {
        println!("{}: {} ({})",
            account.id,
            account.name,
            account.industry.as_deref().unwrap_or("N/A")
        );
    }

    // Create a new contact
    let contact = serde_json::json!({
        "FirstName": "Jane",
        "LastName": "Smith",
        "Email": "jane.smith@example.com",
    });

    let response = client.rest()
        .create("Contact", &contact)
        .await?;

    println!("Created contact: {}", response.id);

    Ok(())
}
```

---

**Document Version:** 1.0
**Last Updated:** 2026-02-07
**Next Review:** After Phase 1 completion
