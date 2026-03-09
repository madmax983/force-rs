# ADR-018: Optimize JwtClaims Lifetimes

**Status:** Proposed
**Date:** 2026-03-05
**Deciders:** Codex, Bolt
**Context:**
The `JwtClaims` struct in `crates/force/src/auth/jwt_bearer.rs` previously used owned `String` fields (`iss`, `sub`, `aud`) for its claims. Because this struct is instantiated every time `JwtBearerFlow::authenticate` is called (which in turn calls `generate_jwt`), the owned `String` fields required cloning the strings from the `JwtBearerFlow` struct on every JWT generation. This caused unnecessary heap allocations, degrading performance, especially under high concurrency or when frequent re-authentication is required.

**Decision:**
We have decided to optimize the `JwtClaims` struct by replacing the owned `String` fields with string slices (`&'a str`) tied to a lifetime parameter `'a`.
This allows the claims to borrow the string data directly from the `JwtBearerFlow` struct without any cloning or heap allocation.

**Consequences:**
### Positive
-   **Performance:** Eliminates unnecessary heap allocations and string cloning during JWT generation, resulting in faster and more memory-efficient execution.
-   **Zero-Cost Abstraction:** Aligns with Rust's philosophy of zero-cost abstractions by leveraging lifetimes for safe borrowing.

### Negative
-   **Complexity:** Introduces a lifetime parameter (`'a`) to the `JwtClaims` struct, which marginally increases the complexity of the struct definition. However, since `JwtClaims` is an internal (`private`) struct used solely within the `jwt_bearer.rs` module for serialization, this complexity does not bleed into the public API or affect consumers.
