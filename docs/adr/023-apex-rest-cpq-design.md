# ADR-023: Apex REST and CPQ API Design

## Status

Accepted

## Context

The Salesforce CPQ (Configure, Price, Quote) API is one of the most widely-used
API surfaces in the Salesforce ecosystem. Unlike standard platform APIs which
route through `/services/data/vXX.0/`, the CPQ API operates through Apex REST
endpoints at `/services/apexrest/SBQQ/ServiceRouter`.

Custom Apex REST endpoints are extremely common in production Salesforce orgs.
Any custom Apex class annotated with `@RestResource` exposes endpoints under
`/services/apexrest/`. Supporting this URL scheme generically benefits all
force-rs users, not just those using CPQ.

## Decision

### Two-Feature Layering

We introduce two feature flags with a dependency relationship:

```toml
apex_rest = []           # Generic Apex REST handler
cpq = ["apex_rest"]      # Typed CPQ operations on top
```

**`apex_rest`** provides `ApexRestHandler<A>` with public HTTP methods
(`get`, `post`, `patch`, `put`, `delete`) for any `/services/apexrest/{path}`
endpoint. This is a user-facing API — callers use it directly to reach custom
Apex REST classes in their orgs.

**`cpq`** provides `CpqHandler<A>` with typed methods for the full CPQ
ServiceRouter surface: quote lifecycle, product loading, configuration,
document generation, and contract amendment.

### URL Resolution on Session

A new `resolve_apex_rest_url(path)` method on `Session` constructs:

```
{instance_url}/services/apexrest/{path}
```

This is fundamentally different from `resolve_url(path)` which constructs:

```
{instance_url}/services/data/{api_version}/{path}
```

The key difference: **Apex REST URLs have no API version prefix**. Apex REST
endpoints are version-less — the versioning is managed by the Apex class itself.

We placed this resolver on `Session` (not on individual handlers) because the
URL pattern is structural, not handler-specific. Both `ApexRestHandler` and
`CpqHandler` use it. The method is feature-gated behind `apex_rest`.

**Alternative considered:** Putting the resolver on each handler individually
(like `UiHandler::resolve_ui_url`). Rejected because the UI API resolver wraps
`Session::resolve_url` (just prepending `ui-api/`), while Apex REST has a
completely different URL structure that cannot delegate to `resolve_url`.

### ServiceRouter Dispatch Pattern

The CPQ API uses an RPC-style dispatch through a single endpoint:

```
POST /services/apexrest/SBQQ/ServiceRouter?loader=SBQQ.QuoteAPI.QuoteReader
```

All operations hit the same URL, differentiated by:
1. The `loader` query parameter (selects the API class)
2. The HTTP method (POST for most operations, PATCH for calculate)

`CpqHandler` abstracts this with two internal dispatch helpers:
- `service_router_post<T>(loader, body)` — for most operations
- `service_router_patch<T>(loader, body)` — for `QuoteCalculator`

### Double-Serialized JSON Bodies

The ServiceRouter expects a request envelope where the `model` field is a
**JSON string** (stringified JSON), not a JSON object:

```json
{
  "saver": "SBQQ.QuoteAPI.QuoteSaver",
  "model": "{\"Id\":\"a0x...\",\"SBQQ__Status__c\":\"Draft\"}"
}
```

`ServiceRouterRequest::new(saver, model)` handles this by calling
`serde_json::to_string()` on the inner model before embedding it in the
outer envelope. This double-serialization is the most error-prone aspect
of the CPQ API and is handled transparently by the handler.

### Type Strategy: Core Fields + Flatten

CPQ models (`QuoteModel`, `QuoteLineModel`, `ProductModel`,
`ConfigurationModel`) use typed fields for the most-used properties plus
`#[serde(flatten)] extra: HashMap<String, Value>` for the long tail.

This matches the pattern established by the UI API (ADR-020) and provides:
- Autocomplete and type safety for common fields
- Zero-maintenance compatibility when Salesforce adds new fields
- Custom field support without any code changes

### CpqHandler Uses Session Directly

`CpqHandler` wraps `Arc<Session<A>>` directly rather than wrapping
`ApexRestHandler`. Both handlers share the same URL resolver on `Session`,
but `CpqHandler` has specialized dispatch logic (the ServiceRouter with
loader parameters) that doesn't map to `ApexRestHandler`'s generic HTTP
methods.

## Consequences

### Positive

- **Generic Apex REST support** — every org with custom Apex REST classes
  gets first-class client support
- **Clean layering** — CPQ builds on Apex REST without tight coupling
- **Consistent patterns** — follows the same handler/Session/feature-gate
  architecture as all other API surfaces
- **Type-safe CPQ** — core fields are typed while remaining extensible

### Negative

- **Session grows** — `resolve_apex_rest_url` adds a method to Session
  (mitigated by feature-gating)
- **Double serialization complexity** — the JSON-in-JSON pattern requires
  careful handling in `ServiceRouterRequest`

## Related

- [ADR-006](006-handler-pattern.md) — Handler pattern for API organization
- [ADR-020](020-ui-api-design.md) — UI API handler with flatten extras pattern
- [ADR-022](022-data-cloud-api-design.md) — Data Cloud API decorator design
