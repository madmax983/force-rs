# DX Audit

**Auditor:** Echo 🗣️
**Date:** 2024-05-23
**Scope:** README.md examples

## Findings

### 1. Quick Start Example
- **Issue:** `ClientCredentials::new` signature mismatch.
  - **Expected:** 2 arguments (`client_id`, `client_secret`)
  - **Actual:** 3 arguments (`client_id`, `client_secret`, `token_url`)
  - **Impact:** Compilation error.
- **Issue:** `client.rest().query_typed` does not exist.
  - **Expected:** `client.rest().query_typed(...)`
  - **Actual:** `client.query(...)`
  - **Impact:** Compilation error.

### 2. Bulk Insert Example
- **Issue:** `ClientCredentials::new` signature mismatch.
  - **Expected:** 2 arguments (`client_id`, `client_secret`)
  - **Actual:** 3 arguments (`client_id`, `client_secret`, `token_url`)
  - **Impact:** Compilation error.

### 3. Bulk Query Example
- **Issue:** `ClientCredentials::new` signature mismatch.
  - **Expected:** 2 arguments (`client_id`, `client_secret`)
  - **Actual:** 3 arguments (`client_id`, `client_secret`, `token_url`)
  - **Impact:** Compilation error.
- **Issue:** `bulk_query_typed` does not exist.
  - **Expected:** `bulk_query_typed`
  - **Actual:** `bulk_query`
  - **Impact:** Compilation error.
- **Issue:** `futures::StreamExt` import is unnecessary.
  - **Reason:** `BulkQueryStream` does not implement `Stream` (uses inherent `next()` method).
  - **Impact:** Potential confusion and unused import warning.

## Recommendations
- Update `README.md` examples to match the current API.
- Ensure `ClientCredentials::new` is called correctly with 3 arguments.
- Replace `query_typed` with `query`.
- Replace `bulk_query_typed` with `bulk_query`.
- Remove unnecessary `futures::StreamExt` import if not used.
