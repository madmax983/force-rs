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
**Threat:** The `BulkHandler::create_job` and `IngestJob::create` accepted arbitrary strings for `object` and `external_id_field_name` without validation. Although `serde_json` handles escaping, sending invalid identifiers (e.g., containing semicolons or dots where not allowed) violates the principle of fail-fast and could potentially be exploited if the server-side validation is insufficient or if these values are reflected in logs/errors.
**Defense:** Enforced strict validation using `validate_sobject_name` and `validate_external_id_field` in `BulkHandler::create_job` and `IngestJob::create`. This ensures only valid Salesforce identifiers are transmitted.

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

## 2024-03-05 - [DoS via Unbounded Error Response Reading]
**Threat:** The `response_to_force_error` function originally read the entire HTTP response body into memory using `response.text().await`. A malicious or compromised Salesforce API server (or MITM attacker if TLS validation was disabled) could return a multi-gigabyte error payload, causing unbounded memory allocation and an Out-Of-Memory (OOM) panic, successfully performing a Denial of Service (DoS) attack.
**Defense:** Replaced the unbounded `response.text().await` with a bounded stream reader (`response.bytes_stream()`) that collects up to 1MB of bytes. Any error payload exceeding 1MB is safely truncated, and `String::from_utf8_lossy` is used to prevent panic on split multi-byte characters at the boundary.
**2024-05-24 - [Unbounded memory allocation during HTTP error response parsing]
**Threat:** A Denial of Service (DoS) vulnerability via memory exhaustion. In client_credentials and jwt_bearer authenticators, the `response.text().await` call unbounded memory allocations reading error payloads. A malicious or misconfigured server returning a multi-gigabyte error body could crash the application.
**Defense:** Replaced unbounded `.text().await` with a 1MB capped stream reader via `response.bytes_stream()` paired with `String::from_utf8_lossy()` to safely bound memory usage while parsing Salesforce error responses.
**2026-03-22 - Prevent Memory Exhaustion DoS in HTTP Error Parsing
**Threat:** Maliciously large chunks from Salesforce API responses could bypass previous byte.len() > limit checks, allowing an unbounded `extend_from_slice` to exhaust memory causing Denial of Service.
**Defense:** Created `read_capped_body` which uses `saturating_sub` to calculate remaining bounds and slices the chunk via `&chunk_bytes[..remaining]` to strictly cap the buffer allocation. Applied to all authenticators and base HTTP error parser.

## 2025-03-26 - [Unmaintained Dependency: rustls-pemfile]
**Threat:** The `force-pubsub` crate pulled in `rustls-pemfile` via `tonic`'s `tls` and `tls-roots` features. `rustls-pemfile` is unmaintained (RUSTSEC-2025-0134) and poses a supply chain risk.
**Defense:** Removed `tls` and `tls-roots` features from `tonic` in `crates/force-pubsub/Cargo.toml` and replaced them with `tls-webpki-roots`. Ran `cargo update -p tonic` to remove the vulnerable dependency and pull in `webpki-roots`.
## 2026-03-27 - [DoS via Unbounded Allocation in Composite Graph]
**Threat:** `CompositeGraphRequest::add_graph` allowed adding an unlimited number of graphs and subrequests. An attacker could construct a request with millions of subrequests, causing unbounded memory consumption before `execute()` is called, leading to an OOM crash.
**Defense:** Updated `add_graph` to return `Result<Self>` and strictly enforce the Salesforce API limit of 500 total subrequests across all graphs. Attempts to add a graph that exceeds this limit now return `ForceError::InvalidInput`.
