# Sentry's Journal

## [Exponential Backoff Overflow]
**Learning:** `u128::pow` panics on overflow, and simple multiplication can also overflow. In `exponential_backoff`, a retry loop with a large attempt count (e.g. 200) caused a panic because $500 * 2^{200}$ exceeds `u128::MAX`. Even though the result was intended to be capped at 30 seconds, the intermediate calculation overflowed before the cap was applied.
**Action:** Always check input bounds before exponentiation or use saturating/checked arithmetic, especially when inputs are driven by loops or configuration. Use early returns for capped values to avoid unnecessary large number calculations.

## [Double-Prefixing URLs]
**Learning:** `query_more` assumed `nextRecordsUrl` was always relative, unconditionally prepending the instance URL. This failed when Salesforce (or a mock) returned an absolute URL, resulting in `https://instance...https://instance...`.
**Action:** Always check if a URL is already absolute (e.g., `starts_with("http")`) before prepending a base URL. Use `reqwest::Url::parse` or manual checks to handle both cases gracefully.

## [Backoff Cap Logic]
**Learning:** `exponential_backoff` logic `min(calculated, MAX)` unconditionally capped the delay at 30s, ignoring the user-configured `base_backoff`. If a user set `base_backoff` to 60s, the client would still retry every 30s.
**Action:** Ensure that capping logic respects the user's minimum configuration. `max(base, MAX)` ensures the delay is never shorter than the base backoff.

## [Timestamp Precision Loss]
**Learning:** `parse_issued_at` used integer division by 1000 to convert milliseconds to seconds, discarding sub-second precision. This could lead to incorrect token expiration calculations (off by up to 1 second).
**Action:** Use `DateTime::from_timestamp_millis` or similar high-precision constructors when parsing timestamps to preserve fidelity.

## [Silent Data Loss on Zero Batch Size]
**Learning:** Functions that batch data (like `process_csv_batches`) must explicitly reject `batch_size: 0`. The iterator logic `0..0` creates an empty range, leading to immediate termination (`None`) which was interpreted as "successful completion" rather than "invalid configuration". This caused silent data loss where records were simply ignored.
**Action:** Always validate configuration parameters like `batch_size`, `concurrency`, etc., against 0 if 0 is not a valid state (like "infinite"). Use `NonZeroUsize` where applicable or explicit checks.

## [BatchBuilder URL Encoding Blindspot]
**Learning:** `BatchBuilder::add_request` accepts raw `&str` URLs and includes them directly in the composite request body without validation or encoding. This differs from `query()` which handles encoding. Users might pass `query?q=Select Id From Account` with spaces, which Salesforce might reject as invalid JSON/URL.
**Action:** When adding "raw" methods to builders, document whether inputs are treated as raw or encoded, and add tests verifying this behavior to prevent accidental regressions or assumptions.
