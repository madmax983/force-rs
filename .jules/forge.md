**[Extract send_request_and_decode helper]**
**Learning:** Consolidating HTTP request execution, error handling, and JSON decoding into a single helper (`send_request_and_decode`) significantly reduces boilerplate and enforces consistent error handling across different API handlers.
**Action:** Look for similar patterns in other modules (e.g., Bulk API) where request execution might be duplicated.

**[Decompose Complex Retry Loops]**
**Learning:** Extracting individual steps of a complex retry loop (execution attempt, error handling, backoff) into dedicated helper methods significantly improves readability and removes the need for clippy suppressions.
**Action:** Identify other complex loops or match statements (e.g. in Bulk API streaming) and decompose them similarly.
