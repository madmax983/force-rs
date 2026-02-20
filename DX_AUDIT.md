# DX Audit

**Auditor:** Echo 🗣️
**Date:** 2024-05-24
**Scope:** README.md examples, crates/force/examples, and API ergonomics

## Findings

### 1. Examples in `crates/force/examples/README.md`
- **Issue:** `ClientCredentials::new` signature mismatch.
  - **Expected:** 2 arguments (`client_id`, `client_secret`)
  - **Actual:** 3 arguments (`client_id`, `client_secret`, `token_url`)
  - **Impact:** Compilation error for users copy-pasting from examples README.
  - **Resolution:** Updated example to use `ClientCredentials::new_production(client_id, client_secret)`.

### 2. Missing Prelude
- **Issue:** Users must import multiple modules (`force::auth::*`, `force::client::*`, `force::error::*`) to get started.
  - **Impact:** Verbose imports and friction for new users.
  - **Resolution:** Added `force::prelude` module exporting `ForceClient`, `ForceClientBuilder`, `ClientCredentials`, `ForceError`, and `ForceResult`.

### 3. Bulk Query API naming
- **Issue:** Previous audit mentioned `bulk_query_typed` missing.
  - **Reality:** The API provides `client.bulk().query<T>(soql)` which is typed and ergonomic. The method name is `query`, not `bulk_query_typed` (though `bulk_query` legacy alias exists).
  - **Resolution:** Verified that `query<T>` works as expected. No code change needed.

### 4. Main README.md
- **Status:** Verified that "Quick Start", "Bulk Insert", and "Bulk Query" examples in the main `README.md` are correct and compile.

## Recommendations
- Use `force::prelude::*` in future examples to simplify imports.
- Ensure all README code blocks are tested in CI (e.g., via doc tests or `dx_test_app`).
