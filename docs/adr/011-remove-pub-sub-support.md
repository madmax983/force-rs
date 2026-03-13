# ADR-011: Remove Pub/Sub Support

**Status:** Accepted
**Date:** 2024-05-22
**Deciders:** Codex, Atlas

**Context:**

The Salesforce Pub/Sub API requires gRPC and Protobuf support, which introduces heavy dependencies (`tonic`, `prost`) and complex build requirements. The current implementation in `force` was experimental and not fully matured. Keeping it in the main crate complicates the build process and increases compile times for users who primarily need REST and Bulk APIs.

**Decision:**
We will remove the `pub_sub` feature and the corresponding `api/pub_sub.rs` module from the `force` crate.

**Consequences:**
### Positive
- **Performance:** Significantly reduced dependency footprint and compile times.
- **Simplicity:** Simplified build process (no `protoc` dependency).

### Negative
- **Features:** No support for Salesforce Pub/Sub API in the core crate. This may be re-introduced later as a separate crate (e.g., `force-pubsub`).
