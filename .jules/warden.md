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

## 2026-02-28 - [Field Name Validation Weakness]
**Threat:** The `validate_field_name` function allowed consecutive dots (`..`) and leading/trailing dots in field paths (e.g., `Parent..Name`), which could lead to malformed SOQL queries or unexpected behavior in Salesforce's parser.
**Defense:** Updated `validate_field_name` to strictly reject field names starting/ending with `.` or containing `..`. Added comprehensive unit tests to `types/validator.rs`.
