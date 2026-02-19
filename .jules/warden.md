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
