**[Extracted SOQL Query Builder]
**Tangle:** The `SoqlQueryBuilder` was located in `crates/force/src/api/rest/soql.rs`, which created an inverted dependency where sibling modules like `composite` (specifically `BatchBuilder`) had to reach into the `rest` module to construct queries.
**Blueprint:** Moved `SoqlQueryBuilder` to the root `api` module (`crates/force/src/api/soql.rs`) to establish a clear structural boundary and eliminate the leaky abstraction. The `rest` module now re-exports it to preserve backward compatibility.
