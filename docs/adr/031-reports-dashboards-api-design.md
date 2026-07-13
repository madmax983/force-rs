# ADR-031: Reports & Dashboards (Analytics) API Design

## Status

Accepted

## Context

Salesforce exposes the **Reports and Dashboards REST API** for running,
describing, and inspecting reports and dashboards programmatically. All of its
endpoints live under a single versioned prefix,
`/services/data/vXX.X/analytics/`, and authenticate with standard OAuth (the
same bearer token the rest of the client already manages):

1. **Reports** (`analytics/reports/...`, `analytics/reportTypes/...`) — run a
   report synchronously with its saved metadata or dynamic overrides, run it
   asynchronously via instances, list/fetch/delete instances, describe a
   report, list reports, run an ad-hoc query without a saved report, and list
   or describe report types.
2. **Dashboards** (`analytics/dashboards/...`) — list dashboards, describe a
   dashboard's layout metadata, read its last-cached component results, refresh
   it, and poll its refresh status.

The responses are JSON with a deeply nested, format-dependent shape: a report
run returns a `factMap` keyed by grouping-coordinate strings (e.g. `"T!T"`,
`"0!T"`), recursive `groupings` trees, and a `reportMetadata` object that is
*also* accepted as a request body when overriding filters/groupings at run
time. Dashboard component internals vary by dashboard type (Lightning vs.
classic) and API version.

## Decision

### Single Feature Flag

```toml
analytics = []
```

Reports and dashboards are bundled under one `analytics` flag because:

- Both surfaces live under the same `/analytics/` URL prefix and are two halves
  of one official product ("Reports and Dashboards REST API").
- Dashboards are composed of report results — the `factMap`/`groupingsDown`
  types used by reports are reused inside dashboard component data.
- They share the same handler, URL resolver, and HTTP helpers; splitting would
  add feature-flag overhead without decoupling anything real.

The flag is a bare empty feature (like `ui`, `graphql`, `consent`) — it pulls
in no extra dependencies. It is added to `full` (and therefore transitively to
`all`).

### `client.analytics()` Naming

The accessor is `client.analytics()` returning `AnalyticsHandler<A>` rather than
`reports()`/`dashboards()`. The name matches the `analytics/` URL segment and
the single feature flag, and keeps both report and dashboard operations under
one discoverable namespace.

### AnalyticsHandler with Standard URL Resolution

`AnalyticsHandler<A>` wraps `Arc<Session<A>>`, manually implements `Clone`
(Arc clone), and exposes a `pub(crate) fn new` — the same shape as
`UiHandler`/`ConsentHandler`. It resolves paths with `Session::resolve_url()`
under an `analytics/` prefix, so no custom URL machinery is needed; the standard
`/services/data/{version}/` versioning is inherited. Private `get`/`post`/
`post_empty`/`put_empty`/`delete_empty` helpers mirror the UI handler, adding a
`put_empty` (dashboard refresh is a bodyless `PUT`) and a `post_empty`
(async instance creation is a bodyless `POST`) plus optional query-parameter
support for `includeDetails`.

### Typed Core, `serde_json::Value` Escape Hatches

The load-bearing envelope is strongly typed so callers get compile-time field
access and enum safety on the parts they actually consume:

- `ReportResults` (`attributes`, `allData`, `factMap`, `groupingsDown/Across`,
  `hasDetailRows`, `reportMetadata`, `reportExtendedMetadata`).
- `FactMapEntry` → `SummaryValue` / `ReportRow` → `DataCell`; recursive
  `Grouping`; `ReportMetadata` with typed `ReportFilter`, `FilterOperator`,
  `ReportFormat`, `GroupingInfo`, `StandardDateFilter`.
- `ReportInstance` + `InstanceStatus`, `ReportDescribe`, `ReportListItem`,
  `ReportTypeCategory`, `DashboardListItem`, `DashboardResults`,
  `DashboardStatus`.

The deeply nested, version-variable long tail is kept as `serde_json::Value`:
cell/aggregate `value` fields (typed by column, and nullable), the
`reportExtendedMetadata` inner descriptors, `reportTypeMetadata`, dashboard
`componentData` entries, chart/property blobs, and `describe_dashboard`'s whole
body. Every typed struct also carries a `#[serde(flatten)] extra` catch-all so
unmodeled fields survive round-trips instead of causing a parse failure. This
keeps the client resilient to API-version drift while still giving strong types
where they matter.

`ReportMetadata` is deliberately dual-purpose: every field is `Option`/`Vec`
with `skip_serializing_if`, and it derives `Default`, so it deserializes from a
report-run response *and* serializes as a compact partial override body. A thin
`ReportMetadataRequest { report_metadata }` wrapper (with `new` and a `From`
impl) models the `{ "reportMetadata": { ... } }` envelope the run-with-metadata
and query endpoints expect.

`FilterOperator` models the known Analytics operators as explicit variants but
preserves any unrecognized operator in an `Other(String)` variant (via a custom
`Serialize`/`Deserialize`), so it is safe in both request and response
positions. `ReportFormat` and `InstanceStatus` use `#[serde(other)]` `Unknown`
fallbacks so new enum values never break parsing.

### No Custom Error Type

The Analytics endpoints return standard Salesforce API error arrays. No bespoke
error envelope is needed — non-2xx responses surface as `ForceError::Api` /
`ForceError::Http`, and JSON decode failures as `ForceError::Serialization`,
via the shared `Session` helpers.

## Consequences

### Positive

- One discoverable `client.analytics()` namespace covers all report and
  dashboard operations behind a single, dependency-free feature flag.
- Callers get typed, ergonomic access to the load-bearing report envelope
  (fact map, groupings, metadata, instances) with fail-safe enums.
- `serde_json::Value` escape hatches plus `flatten` `extra` maps make the client
  tolerant of API-version drift and format-specific fields.
- `ReportMetadata` round-trips, so dynamic run overrides and ad-hoc queries use
  the same type the API returns.

### Negative

- Deeply nested column descriptors, `reportTypeMetadata`, and dashboard
  component internals are `Value` — callers must index into raw JSON for those
  (a deliberate typed-core / raw-tail trade-off).
- Async report runs and dashboard refreshes expose raw create/status endpoints
  without a built-in poller; polling strategy is left to the caller.

## Related

- [ADR-006](006-handler-pattern.md) — Handler pattern for API organization
- [ADR-020](020-ui-api-design.md) — UI API handler design + flatten extras pattern
- [ADR-024](024-consent-portability-api-design.md) — Single-flag, no-custom-error handler precedent
