# Sentry's Journal

## [Exponential Backoff Overflow]
**Learning:** `u128::pow` panics on overflow, and simple multiplication can also overflow. In `exponential_backoff`, a retry loop with a large attempt count (e.g. 200) caused a panic because $500 * 2^{200}$ exceeds `u128::MAX`. Even though the result was intended to be capped at 30 seconds, the intermediate calculation overflowed before the cap was applied.
**Action:** Always check input bounds before exponentiation or use saturating/checked arithmetic, especially when inputs are driven by loops or configuration. Use early returns for capped values to avoid unnecessary large number calculations.

## [Double-Prefixing URLs]
**Learning:** `query_more` assumed `nextRecordsUrl` was always relative, unconditionally prepending the instance URL. This failed when Salesforce (or a mock) returned an absolute URL, resulting in `https://instance...https://instance...`.
**Action:** Always check if a URL is already absolute (e.g., `starts_with("http")`) before prepending a base URL. Use `reqwest::Url::parse` or manual checks to handle both cases gracefully.
