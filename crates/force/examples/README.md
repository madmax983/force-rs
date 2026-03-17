# force-rs Examples

This directory contains complete, runnable examples demonstrating best practices for using the `force` crate to interact with Salesforce APIs.

## Setup

All examples require OAuth credentials set as environment variables:

```bash
export SF_CLIENT_ID="your_client_id_here"
export SF_CLIENT_SECRET="your_client_secret_here"
```

### Getting OAuth Credentials

1. Log in to your Salesforce org
2. Go to Setup → Apps → App Manager
3. Create a New Connected App:
   - Enable OAuth Settings
   - Set Callback URL (e.g., `http://localhost:3000/callback`)
   - Select OAuth Scopes (e.g., `api`, `refresh_token`)
4. Note the **Consumer Key** (Client ID) and **Consumer Secret** (Client Secret)

## Examples

### 1. Organization Limits (`org_limits.rs`)

Demonstrates how to retrieve and display Salesforce org limits with threshold warnings.

**Features:**
- Retrieve API usage, storage, and email limits
- Calculate percentage used
- Display threshold warnings
- Check for limit exhaustion

**Run:**
```bash
cargo run --example org_limits
```

**Use this example to:**
- Monitor API usage in your applications
- Set up alerts for approaching limits
- Understand limit structures

---

### 2. Query Plan Analysis (`query_plan.rs`)

Demonstrates how to use the Query Plan API (`explain`) to inspect query performance costs.

**Features:**
- Analyze SOQL queries before execution
- Inspect cost, cardinality, and operation types (TableScan vs IndexScan)
- View optimization notes and warnings
- **Requires:** `rest` feature enabled (default)

**Run:**
```bash
 cargo run --example query_plan
```

**Use this example to:**
- Optimize SOQL queries
- Prevent inefficient queries in CI/CD
- Understand how Salesforce executes your queries

---

### 3. Basic CRUD Operations (`basic_crud.rs`)

Full lifecycle demonstration of creating, reading, updating, and deleting Salesforce records.

**Features:**
- Create Account and Contact records
- Read records by ID with field access
- Update records with partial data
- Upsert using external IDs
- Delete records (cleanup)
- Error handling patterns

**Run:**
```bash
cargo run --example basic_crud
```

**Use this example to:**
- Learn basic record operations
- Understand parent-child relationships
- See upsert patterns
- Implement cleanup logic

---

### 4. SOQL Queries with Typed Results (`soql_query.rs`)

Demonstrates typed SOQL queries with automatic deserialization and pagination.

**Features:**
- Define typed structs for query results
- Execute queries with type safety
- Handle pagination with `query_more()`
- Work with relationships (subqueries)
- Aggregate queries (COUNT, AVG, GROUP BY)
- Date filters (LAST_N_DAYS)

**Run:**
```bash
cargo run --example soql_query
```

**Use this example to:**
- Build type-safe query layers
- Implement pagination
- Work with complex SOQL queries
- Handle aggregations and grouping

---

### 5. Dynamic Queries (`dynamic_query.rs`)

Demonstrates flexible, untyped queries using `DynamicSObject` for runtime field access.

**Features:**
- Query without predefined types
- Runtime field introspection
- Handle different field types (String, i32, f64, bool)
- Graceful null value handling
- Work with nested/complex data
- Functional processing patterns

**Run:**
```bash
cargo run --example dynamic_query
```

**Use this example to:**
- Build flexible query tools
- Handle unknown schemas
- Implement schema introspection
- Process varied data structures

---

### 6. SOSL Search (`search.rs`)

Multi-object text search using SOSL (Salesforce Object Search Language).

**Features:**
- Search across multiple objects
- Search specific fields (NAME, EMAIL, PHONE)
- Apply filters and limits
- Wildcard searches
- Result grouping by object type

**Run:**
```bash
cargo run --example search
```

**Use this example to:**
- Implement search functionality
- Find records across objects
- Use SOSL efficiently
- Handle multi-object results

---

### 7. Schema Introspection (`describe.rs`)

Explore Salesforce object metadata using the Describe API.

**Features:**
- Global describe (list all objects)
- Object-level metadata (permissions, capabilities)
- Field-level metadata (types, constraints)
- Relationship discovery
- Picklist value enumeration
- Generate SOQL templates
- Capability matrix

**Run:**
```bash
cargo run --example describe
```

**Use this example to:**
- Build schema explorers
- Validate field names
- Generate dynamic UIs
- Create data dictionaries
- Implement field validation

---

### 8. Data Dictionary Generation (`data_dictionary.rs`)

Generate Markdown schema documentation directly from Salesforce describe metadata.

**Features:**
- Export a readable data dictionary for a target SObject
- Optionally include field-usage scanner statistics
- Produce documentation suitable for audits and internal references
- **Requires:** `schema` feature enabled

**Run:**
```bash
cargo run --example data_dictionary --features schema
```

---

### 9. Struct Generator (`generate_struct.rs`)

Generate Rust structs from Salesforce object metadata for typed application code.

**Features:**
- Convert describe metadata into serde-ready Rust structs
- Map Salesforce field types to practical Rust types
- Support live metadata generation and fallback demo mode
- **Requires:** `schema` feature enabled

**Run:**
```bash
cargo run --example generate_struct --features schema
```

---

### 10. SOQL Mass Operations (`soql_mass_op.rs`)

Use SOQL plus Composite Batch to update or delete many records with less boilerplate.

**Features:**
- Query target records once and batch follow-up operations automatically
- Update or delete records in composite-sized chunks
- Track success and failure counts across the run
- **Requires:** `composite` feature enabled

**Run:**
```bash
cargo run --example soql_mass_op --features composite
```

---

## Common Patterns

### Authentication

All examples use the OAuth 2.0 Client Credentials flow:

```rust
use force::auth::ClientCredentials;
use force::client::builder;

// For Sandbox, use: ClientCredentials::new_sandbox("client-id", "client-secret")
let auth = ClientCredentials::new_production(client_id, client_secret);
let client = builder()
    .authenticate(auth)
    .build()
    .await?;
```

### Error Handling

Examples use `anyhow` for error handling:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Your code here
    Ok(())
}
```

In production, use `force::error::Error` for structured error handling:

```rust
use force::error::{Error, Result};

match client.get("Account", &id).await {
    Ok(record) => { /* process record */ }
    Err(Error::HttpError(e)) => { /* handle HTTP error */ }
    Err(e) => { /* handle other errors */ }
}
```

### Field Access Patterns

**Dynamic approach:**
```rust
let result = client.query(soql).await?;
for record in result.records {
    let name: String = record.get_field("Name")?;
    let industry = record.get_field_opt::<String>("Industry")?;
}
```

**Typed approach:**
```rust
#[derive(serde::Deserialize)]
struct Account {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Industry")]
    industry: Option<String>,
}

let result = client.query::<Account>(soql).await?;
for account in result.records {
    println!("{}: {:?}", account.name, account.industry);
}
```

### Pagination

```rust
let mut current_result = client.query(soql).await?;

loop {
    // Process current page
    for record in &current_result.records {
        // ... process record
    }

    // Check for more pages
    if !current_result.has_more() {
        break;
    }

    // Fetch next page
    current_result = client.query_more(&current_result.next_records_url.unwrap()).await?;
}
```

## Building Examples

To compile all examples:
```bash
cargo build --examples
```

To run a specific example:
```bash
cargo run --example <example_name>
```

## Dependencies

Examples use:
- `tokio` - Async runtime
- `serde` - Serialization for typed queries
- `serde_json` - JSON construction
- `anyhow` - Error handling

## Testing Against Real Org

**Important:** Some examples create and delete test data. Use a sandbox or developer org, not production.

Examples that modify data:
- `basic_crud.rs` - Creates and deletes Account/Contact records

Read-only examples:
- `org_limits.rs` - Only reads limits
- `soql_query.rs` - Only queries data
- `dynamic_query.rs` - Only queries data
- `search.rs` - Only searches data
- `describe.rs` - Only reads metadata

## Troubleshooting

### Authentication Errors

If you see authentication errors:
1. Verify your OAuth credentials are correct
2. Check that your Connected App is enabled
3. Ensure your IP is whitelisted (if IP restrictions are set)
4. Verify OAuth scopes include `api`

### Query Errors

If queries fail:
1. Check field names match your org's schema
2. Verify object API names (use describe examples)
3. Check for required field filters
4. Ensure objects are queryable

### Rate Limits

If you hit rate limits:
1. Check limits with `org_limits.rs`
2. Add delays between requests
3. Use bulk operations for large datasets
4. Consider increasing your org's limits

## Next Steps

After exploring these examples:

1. Read the [ADRs](../docs/adr/) to understand design decisions
2. Check the [API documentation](https://docs.rs/force) (once published)
3. Review the [CLAUDE.md](../CLAUDE.md) for architecture overview
4. Explore the source code in `crates/force/src/`

## Contributing Examples

When adding new examples:

1. Follow the existing pattern (setup, examples, cleanup)
2. Use descriptive comments and section headers
3. Include error handling
4. Add example to this README
5. Test against a real Salesforce org
6. Use environment variables for credentials

## License

MIT OR Apache-2.0
