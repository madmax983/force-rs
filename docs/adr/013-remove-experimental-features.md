# ADR-013: Remove Experimental & Nova Features

**Status:** Accepted
**Date:** 2026-02-18
**Deciders:** Atlas, Codex
**Context:** The `force` crate included an `experimental` module and a `nova` feature flag to prototype new API capabilities, specifically `FieldUsageScanner` and `QueryBatch`. These features were intended for internal testing and validation of potential API designs.

Maintaining experimental code within the core crate creates ambiguity for consumers regarding stability and support. Furthermore, the `nova` feature introduced conditional compilation paths that complicated testing and CI without providing tangible value to the stable release.

**Decision:** We will remove the `experimental` module and the `nova` feature flag entirely from the `force` crate.

-   The `crates/force/src/experimental/` directory will be deleted.
-   The `nova` feature will be removed from `Cargo.toml`.
-   Any useful logic from these modules should be evaluated for proper integration into the core API or discarded if no longer relevant.

**Consequences:**

### Positive
-   **API Clarity:** Consumers are presented with a strictly stable API surface.
-   **Maintenance:** Reduced maintenance burden by removing unpolished prototype code.
-   **Build Simplicity:** simplified `Cargo.toml` features and conditional compilation.

### Negative
-   **Breaking Change:** Any consumers relying on `force::experimental::*` will experience a breaking change. However, as the module was explicitly marked experimental and deprecated (in some cases), this is an acceptable trade-off.

## Related ADRs
-   [ADR-004: Feature Gates](004-feature-gates.md)
