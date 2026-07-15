# ADR-029: Account Engagement (Pardot) API v5 Design

**Date:** 2026-07-13
**Status:** Accepted
**Feature flag:** `account_engagement`

## Context

Salesforce Account Engagement (formerly Pardot) exposes a v5 object model REST
API. It is unusual among Salesforce surfaces in three ways:

1. **Separate host.** Requests do NOT target the org instance URL. They target
   `https://pi.pardot.com` (production/training) or `https://pi.demo.pardot.com`
   (sandbox/demo/developer). The path shape is `{host}/api/v5/objects/{object}`
   — there is no `/services/data/vXX.0/` prefix and no API version segment.
2. **Extra required header.** Every request rides the standard Salesforce OAuth
   token (`Authorization: Bearer …`) but must also carry a
   `Pardot-Business-Unit-Id` header (the 18-char `0Uv…` business unit id), and
   the connected app must include the `pardot_api` OAuth scope.
3. **Mandatory `fields` parameter.** Query and read endpoints return no field
   data unless an explicit comma-delimited `fields` parameter is supplied.

Unlike Data Cloud (ADR-022), the auth token itself is unchanged — only the
target host and one header differ. The central send path
(`Session::send_request_and_decode` / `execute_and_check_success`) already
injects the Bearer token regardless of the request URL, so no decorator
authenticator or separate `Session` is needed.

## Decision

### 1. Handler Owns the Host; No `resolve_url`

`AccountEngagementHandler<A>` holds `Arc<Session<A>>` (the plain platform
session), a `business_unit_id: String`, and a `base_url: String`. It builds
absolute URLs itself via `ae_url(path) = "{base_url}/api/v5/{path}"` rather than
calling `Session::resolve_url` (which would produce instance-host,
version-prefixed URLs). Requests are still dispatched through the central send
path so the Bearer token is injected automatically.

### 2. Host Derived from `Environment`, Overridable

The constructor derives `base_url` from the client
[`Environment`](../../crates/force/src/config.rs): `Environment::Sandbox` maps to
`https://pi.demo.pardot.com`; everything else (`Production`, `Custom`) maps to
`https://pi.pardot.com`. `PROD_HOST`/`DEMO_HOST` are exported constants, and a
pure `host_for_environment` function makes the mapping unit-testable.

A public `with_host(base_url)` builder method overrides the host — for a sandbox
custom domain, an org whose demo/prod split doesn't follow the environment enum,
or a mock server in tests.

### 3. Infallible Accessor Taking the Business Unit Id

`ForceClient::account_engagement(business_unit_id)` is hand-written (not via the
`handler_accessor!` macro) because it takes a per-call argument. It is
infallible — no runtime configuration is required beyond the business unit id
that the caller passes in.

### 4. Required `Pardot-Business-Unit-Id` Header

Every helper (`get`/`post`/`patch`/`delete_empty`) sets the
`Pardot-Business-Unit-Id` header from the handler's stored business unit id
before dispatching. This is the one place custom headers are set in this surface.

### 5. Generic Object Operations + Mandatory `fields`

Private generic helpers (`query_objects`/`read_object`/`create_object`/
`update_object`/`delete_object`) keep the typed modules DRY. Every typed
`query_*` method always prepends `("fields", fields)` to the query parameters, so
callers cannot forget the mandatory field list. The `{values, nextPageToken,
nextPageUrl}` envelope is modeled by the generic `QueryResponse<T>`.

### 6. Typed Object Set + Generic Escape Hatch

Seven objects are modeled across six modules, respecting each object's supported
verbs:

- `prospects` — full CRUD
- `lists` and `list-memberships` — full CRUD (memberships path is hyphenated)
- `campaigns` — GET + POST only (no update/delete in v5)
- `custom-fields` — full CRUD (hyphenated path; `type` field mapped to `type_`)
- `forms` — GET / POST / DELETE (no update)
- `emails` — query, read-by-id, and send (POST)

Because Account Engagement has many objects, a public escape hatch
(`get_raw`/`post_raw`/`patch_raw`/`delete_raw`) provides raw `serde_json::Value`
access to any `/api/v5/{path}` endpoint so users are never blocked by an
un-modeled object.

### 7. Integer IDs and Forgiving Structs

v5 uses numeric ids (distinct from 15/18-char Salesforce ids), so `id` and other
numeric ids are `Option<i64>`; `salesforceId` is a separate `Option<String>`.
Because v5 only returns requested fields, every struct field is `Option<...>`
with `#[serde(skip_serializing_if = "Option::is_none")]` (so write bodies omit
unset fields) plus a trailing `#[serde(flatten, default)] extra: HashMap<String,
Value>` for forward-compatible parsing.

### 8. Errors Surface as `ForceError::Http`

The surface reuses the central send path, so non-2xx responses (including the v5
`{"code": …, "message": …}` error body) map through `response_to_force_error`
into `ForceError::Http(HttpError::StatusError { … })`. No bespoke error type is
added — an unused feature-gated error variant would be dead code under
`-D warnings`, and the standard `ForceError::Http` is sufficient.

## Consequences

### Positive

- **Zero changes to `Session` or the auth layer** — only the host and one header
  differ, both handled inside the handler.
- **Bearer token reused** — the standard OAuth token is injected centrally; no
  token exchange or decorator authenticator.
- **Infallible, ergonomic accessor** — `client.account_engagement("0Uv…")` with
  no builder configuration.
- **Never blocked** — the raw escape hatch covers any object not yet typed.
- **Testable host logic** — `host_for_environment` and `with_host` are unit
  tested without network access.

### Negative

- **Environment is an imperfect prod/demo signal** — the true split is driven by
  the connected org's managed-package namespace, not the login environment;
  `with_host` is the escape valve for that case.
- **Business unit id is a stringly-typed argument** — passed on every accessor
  call rather than validated as a newtype.
- **Absolute-URL construction** — the handler bypasses `resolve_url`, so it does
  not benefit from that method's version handling (intentional — v5 has no
  version segment).
