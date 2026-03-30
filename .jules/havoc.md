**[HTTP Memory Exhaustion DoS]**
**The Trigger:** A malicious or overly large HTTP response sent back to a token refresh or API error parser can exhaust memory because of an unbounded `.extend_from_slice` loop.
**The Stack Trace:** No explicit trace, but memory bloats out of control with `A.repeat(50 * 1024 * 1024)` in mock server.
**Reproduction:** Run `cargo fuzz` on HTTP error parser fetching a giant response chunk.
**Comment:** A chunk isn't always bounded by safe dimensions; we must actively truncate `chunk_bytes` before pushing it into the allocation buffer. You assumed chunk bytes could just be pushed and then truncated!
