# Bolt's Journal

**[Optimized SalesforceId Checksum]**
**Learning:** `SalesforceId::compute_checksum` was allocating a `String` on every call, impacting `SalesforceId::new` (validation) and `to_18` (conversion). Returning `[u8; 3]` instead of `String` eliminated this allocation.
**Action:** Always prefer returning fixed-size arrays or using stack buffers for small, fixed-length computations instead of `String` or `Vec`.
