# CPQ API

Typed Salesforce CPQ (Configure-Price-Quote) client: quote lifecycle, product
configuration, document generation, and contract amendment. Requests are
dispatched through the CPQ **ServiceRouter** (a double-serialized JSON
envelope) over Apex REST — so **CPQ layers on the `apex_rest` feature**.

- **Feature flag:** `cpq` (depends on `apex_rest`)
- **Accessor:** `client.cpq()` → `CpqHandler`

```toml
force = { version = "...", features = ["cpq"] }
```

## Quote lifecycle

```rust
let cpq = client.cpq();

let quote      = cpq.read_quote("a0x000000000001AAA").await?;
let calculated = cpq.calculate_quote(&quote).await?;
let saved      = cpq.save_quote(&calculated).await?;

// Add products by ID
use force::api::cpq::AddProductsRequest;
let request = AddProductsRequest::new(vec!["01t000000000001AAA".to_string()]);
let updated = cpq.add_products("a0x000000000001AAA", &request).await?;
```

## Products, config, documents, contracts

```rust
let product  = cpq.load_product("01t000000000001AAA").await?;
let config   = cpq.load_config(/* ... */).await?;
let valid    = cpq.validate_config(&config).await?;
let amended  = cpq.amend_contract("800000000000001AAA").await?;

use force::api::cpq::GenerateDocumentRequest;
let doc_req = GenerateDocumentRequest::new("a0x000000000001AAA")
    .with_template("a0z000000000001AAA")
    .with_format("pdf");
let document = cpq.generate_document(&doc_req).await?;
```

Typed models: `QuoteModel`, `QuoteLineModel`, `ProductModel`,
`ConfigurationModel`.

## See also

- [ADR-023 — Apex REST + CPQ design](../../adr/023-apex-rest-cpq-design.md)
- Rustdoc: `cargo doc --no-deps --features cpq --open` → `force::api::cpq`
