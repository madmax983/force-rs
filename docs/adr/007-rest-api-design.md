# ADR-007: REST API Design Decisions

**Status:** Accepted
**Date:** 2026-02-07
**Deciders:** Mark M, team-lead
**Context:** REST API handler implementation patterns and type design

## Context and Problem Statement

The REST API is the default feature of `force-rs` and provides access to Salesforce's core REST API operations: SOQL queries, CRUD operations, SOSL search, and metadata describe. This is the most commonly used API surface.

**Problem:** How should we design the REST API handler's methods, types, and error handling for maximum safety, ergonomics, and performance?

## Decision Drivers

- **Type safety** - Leverage Rust's type system to prevent errors at compile time
- **Ergonomics** - Easy to use for common cases, powerful for advanced cases
- **Performance** - Zero-copy deserialization where possible, efficient pagination
- **Error handling** - Clear, actionable errors with context
- **Testability** - Easy to mock and test
- **Documentation** - Self-documenting types and clear examples
- **Backward compatibility** - Future-proof API design

## REST API Capabilities

The Salesforce REST API provides:

### 1. SOQL Queries
- **Query** - Execute SOQL queries with automatic pagination
- **Query All** - Include deleted/archived records
- **Query More** - Fetch next page of results
- **Explain** - Get query execution plan

### 2. CRUD Operations
- **Create** - Insert new records
- **Read (Get)** - Retrieve record by ID
- **Update** - Modify existing record
- **Delete** - Remove record
- **Upsert** - Insert or update based on external ID

### 3. SOSL Search
- **Search** - Multi-object text search

### 4. Metadata
- **Describe Global** - List all objects in org
- **Describe** - Get metadata for specific object
- **Describe Layout** - Get page layout metadata

### 5. Limits
- **Limits** - Get org API usage limits

## Considered Options

### Option 1: Untyped JSON Values (Dynamic)
```rust
pub async fn query(&self, soql: &str) -> Result<serde_json::Value> {
    // Returns raw JSON
}

// Usage
let result = client.rest().query("SELECT Id FROM Account").await?;
let records = result["records"].as_array().unwrap(); // Runtime checks
```

**Pros:**
- Simple implementation
- Flexible - works with any query

**Cons:**
- ❌ No compile-time safety
- ❌ Runtime unwraps and type checks
- ❌ No IDE autocomplete for fields
- ❌ Easy to introduce bugs

### Option 2: Fully Typed with Procedural Macros
```rust
#[derive(SObject)]
struct Account {
    #[salesforce(id)]
    id: SalesforceId,
    name: String,
    industry: Option<String>,
}

pub async fn query<T: SObject>(&self, soql: &str) -> Result<Vec<T>> {
    // Deserialize to T
}

// Usage
let accounts: Vec<Account> = client.rest()
    .query("SELECT Id, Name, Industry FROM Account")
    .await?;
```

**Pros:**
- Full type safety
- IDE autocomplete
- Compile-time field validation

**Cons:**
- ❌ Requires derive macro (complex)
- ❌ Tight coupling to schema
- ❌ Hard to handle dynamic queries
- ❌ Schema changes break code

### Option 3: Hybrid Approach (CHOSEN)
```rust
// Generic query returns QueryResult with DynamicSObject
pub async fn query(&self, soql: &str) -> Result<QueryResult<DynamicSObject>> {
    // Returns structured result with dynamic records
}

// Typed query with Into trait
pub async fn query_typed<T: DeserializeOwned>(&self, soql: &str) -> Result<QueryResult<T>> {
    // Deserialize records to T
}

// Usage - Dynamic
let result = client.rest()
    .query("SELECT Id, Name FROM Account")
    .await?;

for record in result.records {
    let id: String = record.get_field("Id")?;
    let name: String = record.get_field("Name")?;
}

// Usage - Typed
#[derive(Deserialize)]
struct Account {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
}

let result: QueryResult<Account> = client.rest()
    .query_typed("SELECT Id, Name FROM Account")
    .await?;

for account in result.records {
    println!("{}: {}", account.id, account.name);
}
```

**Pros:**
- ✅ Flexibility - use dynamic or typed
- ✅ Simple serde derives for typed (no macros)
- ✅ Safe field access with runtime checks
- ✅ Clear upgrade path from dynamic to typed

**Cons:**
- ⚠️ Two query methods
- ⚠️ Runtime overhead for field access in dynamic mode

## Decision Outcome

**Chosen: Option 3 - Hybrid Approach**

We provide both dynamic and typed query methods, allowing users to choose based on their needs:
- `query()` - Returns `QueryResult<DynamicSObject>` for flexible, dynamic queries
- `query_typed<T>()` - Returns `QueryResult<T>` for type-safe, validated queries

### Core Types

#### QueryResult<T>
```rust
/// Result of a SOQL query with pagination support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult<T> {
    /// Total number of records (may exceed records.len() if paginated)
    pub total_size: usize,

    /// Whether there are more records to fetch
    pub done: bool,

    /// Locator for next page (if done == false)
    #[serde(rename = "nextRecordsUrl")]
    pub next_records_url: Option<String>,

    /// Records returned in this page
    pub records: Vec<T>,
}

impl<T> QueryResult<T> {
    /// Check if there are more records to fetch
    pub fn has_more(&self) -> bool {
        !self.done
    }

    /// Get the query locator for the next page
    pub fn next_locator(&self) -> Option<&str> {
        self.next_records_url
            .as_ref()
            .and_then(|url| url.split('/').last())
    }
}
```

#### DynamicSObject
```rust
/// Dynamic SObject with runtime field access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicSObject {
    attributes: Attributes,

    #[serde(flatten)]
    fields: serde_json::Map<String, serde_json::Value>,
}

impl DynamicSObject {
    /// Get the SObject type name
    pub fn object_type(&self) -> &str {
        &self.attributes.r#type
    }

    /// Get a field value with type conversion
    pub fn get_field<T: DeserializeOwned>(&self, name: &str) -> Result<T> {
        self.fields
            .get(name)
            .ok_or_else(|| Error::FieldNotFound(name.to_string()))
            .and_then(|v| serde_json::from_value(v.clone()).map_err(Into::into))
    }

    /// Get a field value as Option (None if missing or null)
    pub fn get_field_opt<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        match self.fields.get(name) {
            None => Ok(None),
            Some(v) if v.is_null() => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(Into::into),
        }
    }

    /// Check if field exists
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    /// Get all field names
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.keys().map(String::as_str).collect()
    }
}
```

#### Attributes
```rust
/// SObject metadata attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes {
    /// SObject API name (e.g., "Account", "Contact")
    pub r#type: String,

    /// REST API URL for this record
    pub url: String,
}
```

### CRUD Operations

#### Create
```rust
/// Create a new record
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let contact_id = client.rest()
///     .create("Contact", &json!({
///         "FirstName": "John",
///         "LastName": "Doe",
///         "Email": "john@example.com"
///     }))
///     .await?;
///
/// println!("Created contact: {}", contact_id);
/// ```
pub async fn create(
    &self,
    sobject_type: &str,
    data: &serde_json::Value,
) -> Result<SalesforceId> {
    // POST /services/data/vXX.0/sobjects/{sobject_type}
    // Returns: { "id": "001...", "success": true }
}

/// Create a record with typed input
pub async fn create_typed<T: Serialize>(
    &self,
    sobject_type: &str,
    record: &T,
) -> Result<SalesforceId> {
    let value = serde_json::to_value(record)?;
    self.create(sobject_type, &value).await
}
```

#### Read (Get)
```rust
/// Get a record by ID
///
/// # Examples
///
/// ```
/// let account = client.rest()
///     .get("Account", &account_id)
///     .await?;
///
/// let name: String = account.get_field("Name")?;
/// ```
pub async fn get(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
) -> Result<DynamicSObject> {
    // GET /services/data/vXX.0/sobjects/{sobject_type}/{id}
}

/// Get a record with typed output
pub async fn get_typed<T: DeserializeOwned>(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
) -> Result<T> {
    // Same endpoint, deserialize to T
}

/// Get specific fields from a record
pub async fn get_fields(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
    fields: &[&str],
) -> Result<DynamicSObject> {
    // GET /services/data/vXX.0/sobjects/{sobject_type}/{id}?fields=Id,Name,Email
}
```

#### Update
```rust
/// Update a record
///
/// # Examples
///
/// ```
/// client.rest()
///     .update("Account", &account_id, &json!({
///         "Industry": "Technology",
///         "NumberOfEmployees": 500
///     }))
///     .await?;
/// ```
pub async fn update(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
    data: &serde_json::Value,
) -> Result<()> {
    // PATCH /services/data/vXX.0/sobjects/{sobject_type}/{id}
    // Returns: 204 No Content on success
}

/// Update a record with typed input
pub async fn update_typed<T: Serialize>(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
    record: &T,
) -> Result<()> {
    let value = serde_json::to_value(record)?;
    self.update(sobject_type, id, &value).await
}
```

#### Delete
```rust
/// Delete a record
///
/// # Examples
///
/// ```
/// client.rest()
///     .delete("Contact", &contact_id)
///     .await?;
/// ```
pub async fn delete(
    &self,
    sobject_type: &str,
    id: &SalesforceId,
) -> Result<()> {
    // DELETE /services/data/vXX.0/sobjects/{sobject_type}/{id}
    // Returns: 204 No Content on success
}
```

#### Upsert
```rust
/// Upsert a record using an external ID
///
/// # Examples
///
/// ```
/// // Upsert using email as external ID
/// let result = client.rest()
///     .upsert(
///         "Contact",
///         "Email__c",
///         "john@example.com",
///         &json!({
///             "FirstName": "John",
///             "LastName": "Doe"
///         })
///     )
///     .await?;
///
/// match result {
///     UpsertResult::Created(id) => println!("Created: {}", id),
///     UpsertResult::Updated(id) => println!("Updated: {}", id),
/// }
/// ```
pub async fn upsert(
    &self,
    sobject_type: &str,
    external_id_field: &str,
    external_id_value: &str,
    data: &serde_json::Value,
) -> Result<UpsertResult> {
    // PATCH /services/data/vXX.0/sobjects/{sobject_type}/{field}/{value}
}

#[derive(Debug)]
pub enum UpsertResult {
    /// Record was created
    Created(SalesforceId),
    /// Record was updated
    Updated(SalesforceId),
}
```

### Query Pagination

```rust
/// Execute a SOQL query with automatic pagination
///
/// # Examples
///
/// ```
/// let mut result = client.rest()
///     .query("SELECT Id, Name FROM Account")
///     .await?;
///
/// println!("Total: {}", result.total_size);
///
/// // Process first page
/// for record in &result.records {
///     let name: String = record.get_field("Name")?;
///     println!("{}", name);
/// }
///
/// // Fetch remaining pages
/// while result.has_more() {
///     result = client.rest()
///         .query_more(&result)
///         .await?;
///
///     for record in &result.records {
///         let name: String = record.get_field("Name")?;
///         println!("{}", name);
///     }
/// }
/// ```
pub async fn query(&self, soql: &str) -> Result<QueryResult<DynamicSObject>> {
    // GET /services/data/vXX.0/query?q={soql}
}

/// Fetch the next page of query results
pub async fn query_more<T>(
    &self,
    previous: &QueryResult<T>,
) -> Result<QueryResult<T>>
where
    T: DeserializeOwned,
{
    // GET {nextRecordsUrl}
}

/// Fetch the next page using a query locator
pub async fn query_more_locator<T>(
    &self,
    locator: &str,
) -> Result<QueryResult<T>>
where
    T: DeserializeOwned,
{
    // GET /services/data/vXX.0/query/{locator}
}
```

### SOSL Search

```rust
/// Execute a SOSL search query
///
/// # Examples
///
/// ```
/// let results = client.rest()
///     .search("FIND {John} IN NAME FIELDS RETURNING Contact(Id, Name)")
///     .await?;
///
/// for search_record in results.search_records {
///     for record in search_record.records {
///         let name: String = record.get_field("Name")?;
///         println!("{}: {}", search_record.sobject_type, name);
///     }
/// }
/// ```
pub async fn search(&self, sosl: &str) -> Result<SearchResult> {
    // GET /services/data/vXX.0/search?q={sosl}
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "searchRecords")]
    pub search_records: Vec<SearchRecords>,
}

#[derive(Debug, Deserialize)]
pub struct SearchRecords {
    #[serde(rename = "attributes")]
    pub attributes: Attributes,

    #[serde(rename = "records")]
    pub records: Vec<DynamicSObject>,
}

impl SearchRecords {
    pub fn sobject_type(&self) -> &str {
        &self.attributes.r#type
    }
}
```

### Describe Operations

```rust
/// Get global describe (all objects in org)
///
/// # Examples
///
/// ```
/// let global = client.rest()
///     .describe_global()
///     .await?;
///
/// for sobject in global.sobjects {
///     if sobject.createable {
///         println!("{}", sobject.name);
///     }
/// }
/// ```
pub async fn describe_global(&self) -> Result<GlobalDescribe> {
    // GET /services/data/vXX.0/sobjects/
}

/// Get metadata for a specific object
///
/// # Examples
///
/// ```
/// let metadata = client.rest()
///     .describe("Account")
///     .await?;
///
/// for field in metadata.fields {
///     println!("{}: {} ({})",
///         field.name,
///         field.label,
///         field.field_type
///     );
/// }
/// ```
pub async fn describe(&self, sobject_type: &str) -> Result<SObjectDescribe> {
    // GET /services/data/vXX.0/sobjects/{sobject_type}/describe/
}
```

### Limits

```rust
/// Get org API limits and usage
///
/// # Examples
///
/// ```
/// let limits = client.rest()
///     .limits()
///     .await?;
///
/// let daily_api = &limits.daily_api_requests;
/// println!(
///     "API Usage: {}/{} ({}% remaining)",
///     daily_api.used,
///     daily_api.max,
///     daily_api.remaining_percent()
/// );
/// ```
pub async fn limits(&self) -> Result<OrgLimits> {
    // GET /services/data/vXX.0/limits/
}
```

## Implementation Structure

```
crates/force/src/api/rest/
├── mod.rs              # RestApi handler + core methods
├── query.rs            # Query types: QueryResult, DynamicSObject
├── crud.rs             # CRUD types: UpsertResult, CreateResponse
├── search.rs           # SOSL types: SearchResult, SearchRecords
├── describe.rs         # Describe types: GlobalDescribe, SObjectDescribe
├── limits.rs           # Limits types: OrgLimits, LimitInfo
└── tests.rs            # Integration tests with wiremock
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    #[error("Field '{0}' not found on record")]
    FieldNotFound(String),

    #[error("Failed to deserialize field '{field}': {source}")]
    FieldDeserializationError {
        field: String,
        source: serde_json::Error,
    },

    #[error("SOQL query error: {0}")]
    QueryError(String),

    #[error("SOSL search error: {0}")]
    SearchError(String),

    #[error("Record not found: {sobject_type} {id}")]
    RecordNotFound {
        sobject_type: String,
        id: String,
    },

    #[error("Pagination locator is invalid")]
    InvalidPaginationLocator,
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_dynamic_sobject_field_access() {
        let sobject = DynamicSObject {
            attributes: Attributes {
                r#type: "Account".to_string(),
                url: "/services/data/v60.0/sobjects/Account/001xx".to_string(),
            },
            fields: serde_json::from_value(json!({
                "Id": "001xx000000ABCD",
                "Name": "Acme Corp",
                "Industry": "Technology",
                "NumberOfEmployees": 500,
            })).unwrap(),
        };

        assert_eq!(sobject.object_type(), "Account");

        let name: String = sobject.get_field("Name").unwrap();
        assert_eq!(name, "Acme Corp");

        let employees: i32 = sobject.get_field("NumberOfEmployees").unwrap();
        assert_eq!(employees, 500);

        assert!(sobject.has_field("Industry"));
        assert!(!sobject.has_field("Website"));
    }

    #[test]
    fn test_query_result_pagination() {
        let result = QueryResult {
            total_size: 250,
            done: false,
            next_records_url: Some("/services/data/v60.0/query/01gxx-2000".to_string()),
            records: vec![],
        };

        assert!(result.has_more());
        assert_eq!(result.next_locator(), Some("01gxx-2000"));
    }
}
```

### Integration Tests (with wiremock)
```rust
#[cfg(test)]
mod integration_tests {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[tokio::test]
    async fn test_query_endpoint() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .and(query_param("q", "SELECT Id FROM Account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    {
                        "attributes": {
                            "type": "Account",
                            "url": "/services/data/v60.0/sobjects/Account/001xx"
                        },
                        "Id": "001xx000000ABCD"
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        // Test with mock client
        // ...
    }
}
```

## Documentation Examples

Every public method includes doc examples following these patterns:

### Basic Example
```rust
/// Execute a SOQL query
///
/// # Examples
///
/// ```
/// let result = client.rest()
///     .query("SELECT Id, Name FROM Account LIMIT 10")
///     .await?;
///
/// for record in result.records {
///     let name: String = record.get_field("Name")?;
///     println!("{}", name);
/// }
/// ```
```

### Advanced Example with Error Handling
```rust
/// # Examples
///
/// ```
/// use force::types::SalesforceId;
///
/// let account_id: SalesforceId = "001xx000000ABCD".parse()?;
///
/// match client.rest().get("Account", &account_id).await {
///     Ok(account) => {
///         let name: String = account.get_field("Name")?;
///         println!("Account: {}", name);
///     }
///     Err(e) => {
///         eprintln!("Failed to get account: {}", e);
///     }
/// }
/// ```
```

## Consequences

### Positive

✅ **Flexible API** - Support both dynamic and typed queries
✅ **Type safety** - Strong types for IDs, versions, etc.
✅ **Ergonomic field access** - `get_field()` with type inference
✅ **Clear pagination** - `has_more()` and `query_more()` methods
✅ **Self-documenting** - Rich type information and doc examples
✅ **Testable** - Easy to mock with wiremock
✅ **Future-proof** - Easy to add new operations

### Negative

⚠️ **Two query methods** - `query()` and `query_typed()` - may confuse users
⚠️ **Runtime field validation** - Dynamic queries check fields at runtime
⚠️ **Memory overhead** - `DynamicSObject` stores full JSON map

### Neutral

ℹ️ **Learning curve** - Users need to understand when to use dynamic vs typed
ℹ️ **Serialization overhead** - Converting between JSON and types
ℹ️ **Field naming** - Salesforce uses PascalCase, Rust prefers snake_case

## Validation

This design will be validated through:
1. Implementation with TDD (RED-GREEN-REFACTOR)
2. Integration tests against real Salesforce org
3. Example programs demonstrating common patterns
4. Documentation review for clarity
5. Performance benchmarks for query and pagination

## Related Decisions

- [ADR-002](002-authentication-strategy.md) - REST handler uses client's authenticator
- [ADR-003](003-error-handling.md) - Error types for REST operations
- [ADR-006](006-handler-pattern.md) - RestApi as a handler object
- Future: ADR for SObject derive macro (optional)

## References

- [Salesforce REST API Developer Guide](https://developer.salesforce.com/docs/atlas.en-us.api_rest.meta/api_rest/)
- [SOQL and SOSL Reference](https://developer.salesforce.com/docs/atlas.en-us.soql_sosl.meta/soql_sosl/)
- [serde documentation](https://serde.rs/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
