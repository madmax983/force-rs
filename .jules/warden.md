# Warden's Journal

**2024-05-24 - SOSL Injection & Bulk Query DoS**
**Threat:**
1. SOSL Injection via unescaped reserved characters in `SearchQueryBuilder::find` method.
2. Denial of Service (DoS) via unbounded memory allocation in `BulkQueryStream` (buffering entire response).

**Defense:**
1. Implemented reserved character escaping in `SearchQueryBuilder` to neutralize injection vectors.
2. Refactored `BulkQueryStream` to use streaming CSV parsing with `csv-async` and `tokio-util`, ensuring bounded memory usage regardless of result size.
