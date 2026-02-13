**2024-05-23 - [Havoc: Access Token Overflow Panic]**
**Threat:** A malicious or corrupted Salesforce OAuth response with an extremely large `expires_in` value combined with a large `issued_at` timestamp causes `chrono::Duration` addition to overflow, leading to a panic and Denial of Service.
**Defense:** Replaced unchecked addition with `checked_add_signed` in `AccessToken::from_response`. If the expiration time overflows `DateTime<Utc>::MAX`, the token is treated as having no expiration.

**2026-02-13 - [Havoc: Bulk Query Memory Exhaustion]**
**Threat:** The `BulkQueryStream` implementation previously buffered the entire CSV response body into memory using `response.text().await` before parsing. For large datasets (typical in Bulk API usage), this could lead to memory exhaustion (OOM) and application crash.
**Defense:** Replaced the buffered implementation with a true streaming approach using `reqwest::Response::bytes_stream()`, `tokio_util::io::StreamReader`, and `csv_async::AsyncReader`. This processes records incrementally without loading the full response into memory.
# Warden's Journal

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder]
**Threat:** The `SearchQueryBuilder` blindly concatenates user input into the SOSL query string without escaping reserved characters. This allows an attacker to inject arbitrary SOSL clauses (e.g., breaking out of the `FIND` clause to modify `RETURNING` objects or access unauthorized fields).
**Defense:** Implement proper escaping for SOSL reserved characters (`? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -`) in the `find` method or during `build`.
