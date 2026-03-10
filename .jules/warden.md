# Warden's Journal

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder]
**Threat:** The `SearchQueryBuilder` blindly concatenates user input into the SOSL query string without escaping reserved characters. This allows an attacker to inject arbitrary SOSL clauses (e.g., breaking out of the `FIND` clause to modify `RETURNING` objects or access unauthorized fields).
**Defense:** Implement proper escaping for SOSL reserved characters (`? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -`) in the `find` method or during `build`.

## 2024-06-25 - [SOQL Injection in Query API]
**Threat:** The `client.rest().query()` method accepts a raw string, encouraging users to construct queries using `format!` with untrusted input. This allows SOQL injection where attackers can break out of string literals or modify query logic.
**Defense:** Implemented `SoqlQueryBuilder` with strict input validation for identifiers and automatic escaping for string literals in WHERE clauses. Added `escape_soql` helper function. Updated documentation to warn against raw string construction.

## 2024-06-25 - [Path Traversal / Injection in Upsert]
**Threat:** The `upsert` operation blindly concatenated `external_id_value` into the URL path. An attacker could supply a value containing `/` or `..` to manipulate the request path or inject arbitrary path segments.
**Defense:** Implemented strict allowlist validation for `sobject` and `external_id_field`. Added URL encoding for `external_id_value` using a custom `AsciiSet` that preserves safe Salesforce characters but encodes separators.

## 2026-02-20 - [Input Validation in Bulk API]
**Threat:** The `BulkHandler::create_job` and `IngestJobBuilder` accepted arbitrary strings for `object` and `external_id_field_name` without validation. Although `serde_json` handles escaping, sending invalid identifiers (e.g., containing semicolons or dots where not allowed) violates the principle of fail-fast and could potentially be exploited if the server-side validation is insufficient or if these values are reflected in logs/errors.
**Defense:** Enforced strict validation using `validate_sobject_name` and `validate_external_id_field` in `BulkHandler::create_job` and `IngestJobBuilder`. This ensures only valid Salesforce identifiers are transmitted.

## 2026-02-21 - [Safe Query Construction in Composite Batch]
**Threat:** `BatchBuilder::add_request` accepts a raw URL string. If users construct this string manually using `format!` with untrusted input (e.g., `format!("query?q=SELECT+Id+FROM+Account+WHERE+Name='{}'", user_input)`), they are vulnerable to SOQL injection or invalid URL formatting (e.g., unencoded spaces).
**Defense:** Added `BatchBuilder::query` which accepts a `SoqlQueryBuilder` and automatically URL-encodes the query string using `form_urlencoded`. Added a warning to `add_request` documentation emphasizing the need for proper URL encoding.

## 2026-02-25 - [DoS via Unbounded Allocation in Bulk Upload]
**Threat:** `IngestJob::upload` accepted `&[u8]` and called `to_vec()`, forcing a heap allocation and copy of the entire payload. For large bulk uploads (up to 150MB), this doubled memory usage and increased the risk of OOM DoS.
**Defense:** Refactored `upload` and internal helpers to accept `impl Into<reqwest::Body>`, allowing zero-copy transmission of `Bytes`, `Vec<u8>`, or streams. Updated internal convenience methods to pass `Vec<u8>` by value instead of reference to avoid cloning.

## 2026-02-26 - [DoS via Unbounded Allocation in Composite Batch]
**Threat:** `BatchBuilder` allowed adding an unlimited number of requests via `add_request` and its convenience wrappers. An attacker could construct a batch with millions of requests, causing unbounded memory consumption before `execute()` is called, leading to an OOM crash.
**Defense:** Updated `add_request`, `query`, and all convenience methods (`get`, `post`, `patch`, `delete`) to return `Result<Self>` and strictly enforce the Salesforce API limit of 25 subrequests. Attempts to add a 26th request now return `ForceError::InvalidInput`.
## 2026-03-01 - [SSRF/Credential Injection in Query Pagination]
**Threat:** The `query_more` method validated that absolute `nextRecordsUrl` values matched the scheme, host, and port of the authenticated `instance_url`. However, it did not restrict embedded credentials (`username` and `password`). An attacker could provide a malicious pagination URL like `https://attacker:password@instance.salesforce.com/...` which would pass the validation but could potentially leak information or cause unexpected authentication behavior.
**Defense:** Fortified `resolve_next_records_url` in `crates/force/src/api/rest/query.rs` to explicitly reject any absolute URLs that contain a username or password.

**2024-05-19 - Fix integer overflow DoS vector in LimitInfo percentage_used calculation**
**Threat:** The `percentage_used` method calculated the used limit by subtracting the `remaining` limit from `max`. If a malicious or malformed response returned unexpected combinations of maximum and remaining limits (e.g., `i64::MIN` and `i64::MAX`), it would cause a subtraction overflow panic. An attacker who could manipulate Salesforce limit responses (e.g. through a proxy or MITM if SSL verification was disabled) or simply an anomaly from Salesforce could reliably trigger this DoS.
**Defense:** Replaced the unsafe unchecked subtraction `self.max - self.remaining` with `self.max.saturating_sub(self.remaining)`, which prevents the overflow and provides bounded behavior without crashing the process.

## 2024-06-25 - [SSRF/Credential Injection via Malicious Relative URLs in query_more]
**Threat:** The `resolve_next_records_url` helper in `query_more` explicitly checked for absolute URLs using `starts_with("http")`. However, standard URL parsers like `url::Url` and `reqwest` treat paths starting with `@` (e.g. `@attacker.com/leak`) or `//` (e.g. `//attacker.com/leak`) as absolute network locations when concatenated with a base URL, completely bypassing the scheme check and allowing an attacker to force the API client to make requests (with bearer tokens) to arbitrary domains.
**Defense:** Fortified `resolve_next_records_url` to explicitly reject any URL that starts with `//` or does not start with either `http` or `/`. This strictly ensures that only safe relative paths or explicitly validated absolute URLs are permitted.
