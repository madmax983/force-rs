# Razor's Journal

## [Reduction]
**Bloat:** `ForceClientBuilder` typestate pattern (`NoAuth`, `HasAuth`, `PhantomData`, `AuthenticatedBuilder`) and `Inner` struct wrapper.
**Cut:** Simplified `ForceClientBuilder` where `build` takes `Authenticator`. Flattened `Inner` fields into `ForceClient`.
**Saved:** Removed ~150 lines of boilerplate. Reduced cognitive load by eliminating builder states and `self.inner` indirection.
