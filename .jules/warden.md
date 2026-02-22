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

## 2026-02-24 - [DoS via Memory Exhaustion in SmartIngest]
**Threat:** `SmartIngest` allowed unbounded `batch_size` (e.g., `usize::MAX`). An attacker or misconfigured client could trigger massive allocations, leading to Denial of Service (DoS) via OOM.
**Defense:** Enforced a hard cap of 50,000 records on `batch_size` in `SmartIngest::batch_size`. This prevents excessive memory allocation while staying well within Salesforce Bulk API limits.

## 2026-02-24 - [Safe Parameter Encoding in Batch API]
**Threat:** `BatchBuilder::add_request` relies on manual URL encoding by the user. Improper encoding could lead to injection or invalid requests.
**Defense:** Added `BatchBuilder::add_request_with_params`, which accepts a map/slice of parameters and safely encodes them using `url::form_urlencoded`. Updated documentation to recommend this safer alternative.

## 2026-02-24 - [DoS via Large Query Strings]
**Threat:** `BulkQueryRequest::new` blindly accepted arbitrarily large query strings. This could be used for memory exhaustion or DoS attacks.
**Defense:** Added validation to `BulkQueryRequest::new` and introduced `try_new`. The constructor now strictly limits queries to 1MB and disallows empty strings.
