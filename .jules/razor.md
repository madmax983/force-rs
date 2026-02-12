## [Reduction]
**Bloat:** `RestHandler` wrapper struct and `.rest()` method
**Cut:** Moved REST methods directly to `ForceClient` and removed `RestHandler`
**Saved:** Reduced API nesting (client.rest().get -> client.get) and removed redundant struct wrapping `Arc<Inner>`.

## [Reduction]
**Bloat:** `ApiVersionSupportTier` enum and `support_tier` method
**Cut:** Deleted the enum and method
**Saved:** Removed unused internal complexity and dead code.

## [Reduction]
**Bloat:** `ForceClientBuilder<Auth>` generic state machine
**Cut:** Simplified to concrete `ForceClientBuilder` and `AuthenticatedBuilder`
**Saved:** Removed `NoAuth` and `HasAuth` structs and complex generic signatures.
