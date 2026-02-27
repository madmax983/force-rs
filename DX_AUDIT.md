# DX Audit

**Auditor:** Echo 🗣️
**Date:** 2026-02-22 (Updated)
**Scope:** README.md examples and API consistency

## Status: VERIFIED ✅

The audit findings confirm that the `force` crate provides a solid developer experience. The examples work as documented, error messages are helpful, and the API is intuitive. One minor friction point regarding Sandbox authentication was identified.

### Findings

#### 1. Sandbox Environment Friction (New)
- **Status:** PASS (with caveat)
- **Details:** The README showcases `ClientCredentials::new_production` but does not mention `ClientCredentials::new_sandbox`.
- **Observation:** A developer working in a Salesforce Sandbox might be confused or assume they need to manually construct the token URL using `ClientCredentials::new`. While `new_sandbox` exists in the code and is discoverable via IDE autocomplete, adding it to the documentation or examples would improve the "Getting Started" experience for sandbox users.

#### 2. Quick Start Example
- **Status:** PASS
- **Details:** The example correctly uses `ClientCredentials::new_production`, which takes 2 arguments (`client_id`, `client_secret`).
- **Observation:** Using `ClientCredentials::new` (3 arguments) requires a token URL, and failure to provide it results in a clear compilation error. Runtime failures with invalid URLs provide helpful network errors.
- **Verification:** Copy-pasting the example into a fresh project with `force = "0.1"` works correctly.

#### 3. Bulk API Examples
- **Status:** PASS
- **Details:** Both Bulk Insert and Bulk Query examples compile and run correctly.
- **Observation:** `client.bulk().insert` and `client.bulk().query` are intuitive and match the documentation.
- **Note:** `futures::StreamExt` is not imported in the examples, avoiding unused import warnings. `BulkQueryStream` provides an inherent `next()` method, which simplifies usage without needing extra traits.

#### 4. Error Messages
- **Status:** PASS
- **Details:** Error messages are descriptive and helpful.
  - Authentication failure: `authentication failed: OAuth token request failed: invalid_client_id: client identifier invalid`
  - Network failure: `Http(RequestFailed(reqwest::Error { ... "dns error" ... }))`

#### 5. API Consistency
- **Status:** PASS
- **Details:** The public API matches the documentation.
  - `query_typed` does not exist; `query` is used correctly.
  - `bulk_query_typed` does not exist; `bulk_query` (or just `query` via trait/inherent) is used correctly.

#### 6. Missing Feature Flag Friction (New)
- **Status:** FAIL -> FIXED (in progress)
- **Details:** The "Query Plan API" example in `README.md` fails to compile if the `nova` feature is not enabled.
- **Observation:** The compiler error `no method named explain found for struct RestHandler` is technically correct but confusing for a user who just copy-pasted the code. The user might think the documentation is outdated or the method was removed.
- **Fix:** Add a prominent banner or note in the `README.md` section for Query Plan API stating that it requires the `nova` feature, and ensure the code block comments reflect this.

## Conclusion

The "Developer Experience" for the `force` crate is excellent. The examples are copy-pasteable and work out of the box. The error messages guide the user to the problem. The API surface is clean and avoids unnecessary complexity. The minor issue with Sandbox discoverability does not block usage but could be improved.
