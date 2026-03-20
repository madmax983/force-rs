# ADR-020: UI API Design

**Date:** 2026-03-20
**Status:** Accepted
**Feature flag:** `ui`

## Context

The Salesforce UI API (`/services/data/vXX.0/ui-api/`) is a distinct API surface that serves layout-aware, presentation-ready data. It differs fundamentally from the REST API:

- **Different URL prefix** — `ui-api/` vs `sobjects/`
- **Different response shapes** — returns display values, layout sections, and field metadata alongside raw values
- **Different endpoint patterns** — no `sobjects/{type}/{id}` pattern; instead `records/{id}`, `object-info/{object}`, `layout/{object}`, etc.

The UI API covers: record CRUD, record-ui aggregation, object metadata, page layouts, list views, record actions, lookup type-ahead, and favorites.

## Decision

### 1. Dedicated handler, not RestOperation

`UiHandler<A>` does **not** implement `RestOperation`. The `RestOperation` trait was designed for the `sobjects/` URL pattern and its CRUD/query/describe operations. The UI API's endpoint structure is incompatible with that trait's assumptions.

### 2. `ui-api/` URL prefix via `resolve_ui_url`

A dedicated helper method in `UiHandler`:

```rust
pub(crate) async fn resolve_ui_url(&self, path: &str) -> Result<String> {
    self.inner.resolve_url(&format!("ui-api/{}", path.trim_start_matches('/'))).await
}
```

This delegates to `Session::resolve_url`, which constructs `{instance_url}/services/data/{version}/{path}`. Adding `ui-api/` as a prefix produces the correct UI API base.

### 3. HTTP helpers on UiHandler

Rather than duplicating HTTP execution boilerplate across all sub-modules, `UiHandler` provides four internal helpers:

- `get(path, query, error_msg) -> Result<T>` — GET + JSON deserialize
- `post(path, body, error_msg) -> Result<T>` — POST + JSON deserialize
- `patch(path, body, error_msg) -> Result<T>` — PATCH + JSON deserialize
- `delete_empty(path, error_msg) -> Result<()>` — DELETE, expect 204

These helpers share the same middleware pipeline (retry, rate limit, token refresh) as all other handlers via `Session::send_request_and_decode` and `Session::execute_request`.

### 4. Sub-module `impl UiHandler<A>` blocks

Methods are organized into resource files (`records.rs`, `object_info.rs`, etc.), each adding an `impl<A: Authenticator> UiHandler<A>` block. This keeps types and their corresponding methods co-located, matching the pattern established by `api/rest/` modules.

### 5. `#[serde(flatten)] pub extra: HashMap<String, Value>` on all response types

The UI API is evolving and returns many undocumented fields. Using `#[serde(flatten)]` for an `extra` catch-all ensures:
- Deserialization never fails on unknown fields
- Callers can still access undocumented data if needed
- Strongly typed fields remain ergonomic for documented properties

### 6. `LayoutType` and `Mode` as enums with `as_str()`

Rather than accepting raw strings for query parameters, the public API uses enums:

```rust
pub enum LayoutType { Compact, Full }
pub enum Mode { Create, Edit, View }
```

Each has an `as_str()` method returning the Salesforce-expected string value. This prevents typos and documents the valid values at the type level.

## Module Structure

```
api/ui/
  mod.rs          UiHandler struct + HTTP helpers + resolve_ui_url
  types.rs        FieldValueRepresentation, LayoutType, Mode
  records.rs      record_ui, get_record, batch, CRUD, defaults (8 methods)
  object_info.rs  object_info, object_infos_batch (2 methods)
  layouts.rs      layout (1 method)
  list_views.rs   list_ui, list_views, list_records, list_info (4 methods)
  actions.rs      record_actions (1 method)
  lookups.rs      lookup, filtered_lookup (2 methods)
  favorites.rs    get/create/update/delete favorites (4 methods)
```

Total: 22 endpoint methods across 8 resource files.

## Consequences

**Positive:**
- Clear separation from REST/Tooling APIs that use `RestOperation`
- Compile-time safety for layout type and mode parameters
- Forward-compatible deserialization via `extra` fields
- Consistent error handling through shared session helpers

**Negative:**
- Does not share `RestOperation` CRUD — duplication with REST for similar record operations (create, update, delete). However, the response shapes are different enough (UI API returns full `RecordRepresentation`; REST returns just the ID on create, 204 on update/delete) that sharing would add complexity.

## Alternatives Considered

**Implement RestOperation** — Rejected. The UI API's record endpoints return different shapes and use different URLs. Forcing them into `RestOperation` would require type gymnastics and make the public API confusing.

**Single flat module** — Rejected. 22 methods + all their types in one file would be over 1000 lines and hard to navigate.

**Trait object instead of generic** — Rejected. Would require heap allocation per call and `dyn Authenticator`. The phantom-type generic pattern is consistent with all other handlers.
