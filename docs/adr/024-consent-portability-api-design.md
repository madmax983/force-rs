# ADR-024: Consent & Portability API Design

## Status

Accepted

## Context

GDPR, CCPA, and other privacy regulations require organizations to manage
consent preferences and fulfill data subject access requests (DSARs).
Salesforce provides two related REST API surfaces for this:

1. **Consent API** (`/consent/`) — Check whether records have consented to
   specific actions like email, tracking, or right-to-be-forgotten.
2. **Portability API** (`/portability/`) — Compile and export personal data
   for a data subject (GDPR Article 20).

Both use standard `/services/data/vXX.0/` URL prefixes with normal OAuth
authentication. Consent *writes* are handled via standard sObject CRUD
(ContactPointTypeConsent, DataUsePurpose, etc.) which already works through
the existing REST handler.

## Decision

### Single Feature Flag

```toml
consent = []
```

Consent reads and portability are bundled in one feature because:
- The surface is small (5 methods total)
- Users who care about consent almost always need portability too
- Both are privacy/compliance motivated
- Splitting would add feature flag overhead for ~2 methods each

### ConsentHandler with Standard URL Resolution

`ConsentHandler<A>` wraps `Arc<Session<A>>` and uses `Session::resolve_url()`
with `consent/` and `portability/` prefixes. No custom URL resolver needed —
these endpoints follow the standard versioned REST pattern.

### Typed ConsentValue Enum

```rust
pub enum ConsentValue {
    Yes,
    No,
    Unknown,
}
```

Unknown API values deserialize to `Unknown` rather than failing. This is a
deliberate compliance-safety choice: treating unknown consent as "probably
granted" is a regulatory violation. The fail-safe default is denial.

### Raw Portability Endpoints (No Built-in Polling)

The portability API is async — POST to compile, GET to check status. We
expose both as raw endpoints without a convenience poller because polling
strategies vary wildly by use case (webhook callbacks, queue-based,
user-facing progress bars). The caller decides.

### No Custom Error Type

The consent and portability endpoints return standard Salesforce API errors.
No `ConsentErrorResponse` type needed — `ForceError::Api` handles everything.

## Consequences

### Positive

- Privacy/compliance features accessible with typed, safe APIs
- `ConsentValue::Unknown` prevents accidental consent assumption
- Small, focused surface — easy to maintain
- No special auth or URL handling needed

### Negative

- Portability polling is left to the caller (deliberate trade-off)
- Consent writes go through `.rest()` rather than `.consent()` (prevents
  duplicating CRUD logic)

## Related

- [ADR-006](006-handler-pattern.md) — Handler pattern for API organization
- [ADR-020](020-ui-api-design.md) — UI API flatten extras pattern (reused here)
