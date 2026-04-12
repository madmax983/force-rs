# Contributing to force-rs

Thank you for your interest in contributing to `force-rs`! This document outlines the development workflow and standards for this project.

## Code of Conduct

We expect all contributors to be respectful and professional in their interactions. This project aims to be welcoming to developers of all experience levels.

## Development Philosophy

### Test-Driven Development (TDD)

This project follows **strict TDD discipline** using the RED-GREEN-REFACTOR cycle:

1. **RED**: Write a failing test that defines the desired behavior
2. **GREEN**: Write the minimal code to make the test pass
3. **REFACTOR**: Clean up the implementation while keeping tests green

**No code should be written without a test driving it.**

### Code Quality Standards

All contributions must meet these quality gates:

- `cargo fmt` - Code must be formatted with rustfmt
- `cargo clippy -- -D warnings` - Zero clippy warnings (pedantic/nursery lints enabled)
- `cargo test` - All tests must pass
- No `unwrap()` or `expect()` in production code (document exceptions in tests)
- Doc comments with examples on all public APIs

## Getting Started

### Prerequisites

- Rust 2024 edition or later
- A Salesforce Developer Edition org for integration testing (optional)

### Building the Project

```bash
# Clone the repository
git clone https://github.com/markm/force-rs.git
cd force-rs

# Build the workspace
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Format code
cargo fmt
```

## Making Changes

### Before You Start

1. Check existing issues or create a new one to discuss your proposed changes
2. Read the relevant Architecture Decision Records (ADRs) in `docs/adr/`
3. Understand the feature gate structure (see ADR-004)

### Development Workflow

1. **Fork the repository** and create a feature branch
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Write tests first** (RED phase)
   - Add tests to the appropriate module in `crates/force/tests/`
   - Use `wiremock` for HTTP mocking
   - Run `cargo test` to verify tests fail for the right reason

3. **Implement the feature** (GREEN phase)
   - Write minimal code to pass the tests
   - Follow existing patterns (see `docs/adr/` for guidance)
   - Run `cargo test` frequently

4. **Refactor** (REFACTOR phase)
   - Extract common patterns (DRY principle)
   - Improve clarity without changing behavior
   - Ensure tests still pass

5. **Quality checks**
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```

6. **Commit your changes**
   - Write clear, concise commit messages
   - Reference issue numbers where applicable

7. **Submit a pull request**
   - Describe what changed and why
   - Link to related issues
   - Ensure CI passes

## Pull Request Guidelines

### Required for All PRs

- All tests must pass
- Code must be formatted (`cargo fmt`)
- Zero clippy warnings (`cargo clippy -- -D warnings`)
- New features must include:
  - Tests demonstrating the feature works
  - Doc comments with examples
  - Updates to relevant ADRs or creation of new ones
- Bug fixes must include:
  - A test reproducing the bug
  - The fix that makes the test pass

### PR Review Process

1. Automated CI checks must pass
2. At least one maintainer review required
3. Address any feedback or requested changes
4. Maintainer will merge once approved

## CI and Release Gates

The project uses layered CI lanes:

- Fast lane: formatting, clippy (`-D warnings`), and unit-focused tests.
- Full lane: broader feature/test matrix.
- Nightly live-contract lane: ignored live Salesforce tests against real org credentials.

All PRs must pass lint/test gates before merge.

## Project Structure

```
force-rs/
├── crates/
│   └── force/           # Main library crate
│       ├── src/
│       │   ├── auth/    # Authentication implementations
│       │   ├── api/     # API handlers (REST, Bulk, etc.)
│       │   ├── types/   # Core type system
│       │   └── error/   # Error hierarchy
│       ├── tests/       # Integration tests
│       └── examples/    # Usage examples
├── docs/
│   └── adr/            # Architecture Decision Records
└── README.md
```

## Architecture Decision Records (ADRs)

Before making significant architectural changes, review existing ADRs:

- [ADR-001: Workspace Structure](docs/adr/001-workspace-structure.md)
- [ADR-002: Authentication Strategy](docs/adr/002-authentication-strategy.md)
- [ADR-003: Error Handling](docs/adr/003-error-handling.md)
- [ADR-004: Feature Gates](docs/adr/004-feature-gates.md)
- [ADR-005: Compile-Time Auth Safety](docs/adr/005-compile-time-auth-safety.md)
- [ADR-006: Handler Pattern](docs/adr/006-handler-pattern.md)
- [ADR-007: REST API Design](docs/adr/007-rest-api-design.md)
- [ADR-008: Bulk API Design](docs/adr/008-bulk-api-design.md)

Significant changes should be accompanied by a new ADR or an update to an existing one.

## Testing Guidelines

### Unit Tests

- Place unit tests in the same file as the code being tested (using `#[cfg(test)]` modules)
- Test edge cases, error paths, and typical usage

### Integration Tests

- Place integration tests in `crates/force/tests/`
- Use `wiremock` to mock Salesforce API responses
- Test complete workflows (e.g., create → read → update → delete)

### Test Naming

- Use descriptive test names: `test_create_returns_valid_id()` not `test_1()`
- Group related tests with module hierarchies

### Example Test Structure

```rust
#[tokio::test]
async fn test_query_returns_records() {
    // RED: Define expected behavior
    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path_regex("/query.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalSize": 1,
            "done": true,
            "records": [{"Id": "001xx000003DGb2AAG", "Name": "Test"}]
        })))
        .mount(&mock_server)
        .await;

    // GREEN: Minimal implementation passes
    let client = setup_test_client(&mock_server).await;
    let result = client.rest().query::<Account>("SELECT Id, Name FROM Account").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().records.len(), 1);
}
```

## Feature Flags

This project uses feature gates extensively (see ADR-004). When adding new functionality:

- Determine if it should be feature-gated
- Update `Cargo.toml` with the new feature
- Document the feature in the README
- Ensure tests cover both feature-enabled and feature-disabled builds

## Documentation

### Doc Comments

All public items must have doc comments:

```rust
/// Creates a new Account record.
///
/// # Arguments
///
/// * `account` - The account data to create
///
/// # Returns
///
/// Returns the Salesforce ID of the created record.
///
/// # Errors
///
/// Returns `ForceError` if the API request fails.
///
/// # Example
///
/// ```no_run
/// # use force::types::Account;
/// # use force::ForceClient;
/// # async fn example(client: &ForceClient<force::auth::ClientCredentials>) {
/// let account = Account { name: "Acme Corp".to_string(), ..Default::default() };
/// let id = client.rest().create("Account", &account).await.unwrap();
/// # }
/// ```
pub async fn create<T>(&self, sobject_type: &str, record: &T) -> Result<SalesforceId>
where
    T: serde::Serialize,
{
    // Implementation
}
```

### Examples

Runnable examples help users understand the API:

- Place examples in `crates/force/examples/`
- Include a `README.md` explaining what each example demonstrates
- Use `// Example output:` comments to show expected results

## Questions?

If you have questions about contributing:

- Open an issue with the `question` label
- Check existing ADRs for architectural guidance
- Review the `examples/` directory for usage patterns

Thank you for contributing to `force-rs`!

## Live Contract Testing

Live tests are in `crates/force/tests/live_salesforce.rs` and use `#[ignore]` by default.

Credential sources:

- explicit environment variables:
  - `SF_ACCESS_TOKEN`
  - `SF_INSTANCE_URL`
- or a locally authenticated Salesforce CLI org discovered via `sf org display --verbose --json`
  - optional `SF_TARGET_ORG` to select a non-default org alias or username
- optional `SF_API_VERSION` (default `v60.0`)

Optional runtime tuning:

- `SF_LIVE_TEST_TIMEOUT_SECS`
- `SF_LIVE_BULK_POLL_MAX_ATTEMPTS`
- `SF_LIVE_BULK_POLL_INITIAL_BACKOFF_MS`
- `SF_LIVE_BULK_POLL_MAX_BACKOFF_MS`
- `SF_LIVE_BULK_QUERY_ROW_LIMIT`

Optional gated scenarios:

- `SF_LIVE_RUN_PARTIAL_FAILURE=1`
- `SF_LIVE_RUN_THROTTLE=1`

Run manually:

```bash
cargo test -p force --all-features --test live_salesforce -- --ignored --test-threads=1
```

Examples:

```bash
# Use explicit env vars
SF_ACCESS_TOKEN=... SF_INSTANCE_URL=https://your-org.my.salesforce.com \
  cargo test -p force --all-features --test live_salesforce -- --ignored --test-threads=1

# Use the default authenticated Salesforce CLI org
cargo test -p force --all-features --test live_salesforce -- --ignored --test-threads=1

# Use a specific authenticated Salesforce CLI org
SF_TARGET_ORG=my-dev-org \
  cargo test -p force --all-features --test live_salesforce -- --ignored --test-threads=1
```
