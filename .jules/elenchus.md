# Elenchus's Journal

## Verdicts & Patterns

| Module | Verdict | Severity | Finding |
|--------|---------|----------|---------|
| `api::rest::query` | 🟢 Acquitted | 🔴 Critical | `query_more` handled absolute URLs correctly after fix. Regression test added. |

## Detailed Findings

**[Query More URL Handling]**
**Module:** `api::rest::query`
**Severity:** 🔴 Critical
**Finding:** The `query_more` function blindly concatenates `instance_url` with `next_records_url`. If Salesforce returns an absolute URL (which is valid and possible), this results in a malformed URL (double scheme/host). Existing tests only mocked relative URLs, mirroring the implementation's assumption.
**Evidence:** `regression_query_pagination.rs` fails with `builder error` when fed an absolute URL.
**Resolution:** `query_more` updated to detect absolute URLs. Unit tests added to `api::rest::query` covering absolute and malformed URLs. Regression test `regression_query_pagination.rs` passes.
