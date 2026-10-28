# Tooling API

REST-style access to development metadata (`ApexClass`, `ApexTrigger`, …) over the
`/services/data/vXX.0/tooling/` prefix. It shares the exact same CRUD / Query /
Describe surface as the [REST API](rest.md) — only the URL prefix differs — and
adds development-only endpoints: execute-anonymous, run-tests, and code
completions.

- **Feature flag:** `tooling` (independent of `rest` — enable it on its own)
- **ApexAccessor:** `client.tooling()` → `ToolingHandler`

```toml
force = { version = "...", features = ["tooling"] }
```

CRUD/Query/Describe come from the shared [`RestOperation`] trait, so bring it into
scope:

```rust
use force::api::RestOperation;
```

## Shared operations (Tooling objects)

Same methods as REST, routed under `/tooling/`:

```rust
let tooling = client.tooling();

let classes = tooling.query::<serde_json::Value>("SELECT Id, Name, Status FROM ApexClass").await?;
let describe = tooling.describe("ApexClass").await?;
tooling.create("ApexClass", &json!({ "Body": "public class Foo {}" })).await?;
```

## Tooling-only endpoints

```rust
use force::api::tooling::{CompletionsType, RunTestsRequest, TestItem};

// Execute anonymous Apex
let exec = tooling.execute_anonymous("System.debug('hi');").await?;
if exec.is_success() { /* ... */ }
// else: exec.is_compile_error() / exec.is_runtime_error()

// Run Apex tests — synchronous (blocks for results) or async (returns a job ID)
let request = RunTestsRequest {
    tests: vec![TestItem { class_id: "01p...".into(), test_methods: None }],
    max_failed_tests: Some(-1),
};
let results = tooling.run_tests(&request).await?;   // RunTestsResult
let job_id  = tooling.run_tests_async(&request).await?; // String

// Code completions
let completions = tooling.completions(CompletionsType::Apex, "System.d").await?;
```

### Winter '27 (v68.0) endpoints

```rust
use force::api::tooling::{
    DiscoveryTestLevel, SymbolCategory, SymbolsRequest, TestDiscoveryRequest,
};

// Compile errors of Apex that is not valid (GA). Does not change IsValid.
let compile = tooling.apex_compile_results().await?;
for failed in &compile.results {
    println!("{}: {} problem(s)", failed.name, failed.problems.len());
}

// Apex type metadata (Beta). One request per org at a time.
let request = SymbolsRequest::new(SymbolCategory::Builtin)
    .namespace("System")
    .name("ApexPages");
let symbols = tooling.symbols(&request).await?;

// Test Discovery. `test_level` needs v68.0+; `show_all_methods` is v67.0 and earlier.
let request = TestDiscoveryRequest {
    test_level: Some(DiscoveryTestLevel::RunLocalTests),
    page_size: Some(500),
    ..Default::default()
};
let mut page = tooling.discover_tests(&request).await?;
while let Some(next) = page.next_records_url.clone() {
    page = tooling.discover_tests_next(&next).await?;
}
```

## See also

- Auth setup: [Choosing an auth flow](../02-choosing-an-auth-flow.md)
- Retries, errors, pagination: [Operations](../03-operations.md)
- [ADR-019 — RestOperation trait & Tooling API design](../../adr/019-tooling-api-design.md)
- Example: [`tooling.rs`](../../../crates/force/examples/tooling.rs)
- Rustdoc: `cargo doc --no-deps --features tooling --open` → `force::api::tooling`

[`RestOperation`]: ../../../crates/force/src/api/rest_operation.rs
