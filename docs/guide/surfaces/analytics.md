# Reports & Dashboards (Analytics) API

Run reports and dashboards and read their results and metadata over the
`/services/data/vXX.X/analytics/` REST endpoints. One feature gates both reports and
dashboards.

- **Feature flag:** `analytics`
- **Handler accessor:** `client.analytics()` → `AnalyticsHandler`

## Reports

Synchronous runs return up to 2000 detail rows; use the async instance flow beyond that.

```rust
// Synchronous run with detail rows.
let results = client.analytics().run_report("00O3000000B5Yn2", true).await?;
let grand_total = &results.fact_map["T!T"].aggregates[0];
println!("total = {}", grand_total.label);

// Asynchronous run: start an instance, then poll for results.
let instance = client.analytics().run_report_async("00O3000000B5Yn2", true).await?;
let done = client.analytics()
    .get_report_instance("00O3000000B5Yn2", &instance.id)
    .await?;
```

`run_report_with_metadata` / `run_report_async_with_metadata` / `query_report` take a
`ReportMetadataRequest` to override filters, groupings, and scope at run time.

| Method | Purpose |
|---|---|
| `run_report(id, include_details)` | Sync run from saved metadata |
| `run_report_with_metadata(id, include_details, &meta)` | Sync run with overrides |
| `query_report(&meta)` | Ad-hoc run (no saved report) |
| `run_report_async(id, include_details)` | Start an async instance |
| `run_report_async_with_metadata(id, include_details, &meta)` | Async instance with overrides |
| `list_report_instances(id)` | List a report's instances |
| `get_report_instance(id, instance_id)` | Fetch instance results |
| `delete_report_instance(id, instance_id)` | Delete an instance |
| `describe_report(id)` | Report describe metadata |
| `list_reports()` | List reports |
| `list_report_types()` | List report-type categories |
| `describe_report_type(report_type)` | Describe a report type |

## Dashboards

```rust
let dash = client.analytics().get_dashboard_results("01Z3000000ABCDE").await?;
let status = client.analytics().refresh_dashboard("01Z3000000ABCDE").await?;
let poll = client.analytics().get_dashboard_status("01Z3000000ABCDE").await?;
```

| Method | Purpose |
|---|---|
| `list_dashboards()` | List dashboards |
| `describe_dashboard(id)` | Dashboard describe (raw JSON) |
| `get_dashboard_results(id)` | Current dashboard results |
| `refresh_dashboard(id)` | Trigger a refresh |
| `get_dashboard_status(id)` | Poll refresh status |

Typed cores (`ReportResults`, `factMap`, `groupings`, `ReportMetadata`) carry
`serde_json::Value` escape hatches for fields not yet modeled; format/status/operator enums
are fail-safe.

## See also

- [ADR-031 — Reports & Dashboards API design](../../adr/031-reports-dashboards-api-design.md)
- Rustdoc: `force::api::analytics` (`AnalyticsHandler`, `ReportResults`, `DashboardResults`, `ReportMetadataRequest`)
