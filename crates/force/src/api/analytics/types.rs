//! Reports & Dashboards (Analytics) API data types.
//!
//! Typed request/response models for the Salesforce Reports and Dashboards REST
//! API. The load-bearing envelope (report-run results, fact map, groupings,
//! report metadata, instances) is strongly typed; the deeply-nested and
//! version-variable long tail (report-type metadata, dashboard component
//! internals, chart/property blobs) is kept as [`serde_json::Value`] escape
//! hatches so the client tolerates schema drift across API versions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Enums ────────────────────────────────────────────────────────────

/// The visual format of a report.
///
/// Unknown values from the API deserialize to [`Unknown`](Self::Unknown)
/// so that new formats do not break parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportFormat {
    /// A flat list of records with no groupings.
    Tabular,
    /// Rows grouped along one or more down-groupings.
    Summary,
    /// Rows and columns grouped (down and across).
    Matrix,
    /// A joined report with multiple blocks.
    MultiBlock,
    /// An unrecognized report format.
    #[serde(other)]
    Unknown,
}

/// The status of an asynchronous report instance (and dashboard/component runs).
///
/// Unknown values deserialize to [`Unknown`](Self::Unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum InstanceStatus {
    /// The instance has been created but has not started running.
    New,
    /// The instance is currently running.
    Running,
    /// The instance completed successfully.
    Success,
    /// The instance failed.
    Error,
    /// An unrecognized status value.
    #[serde(other)]
    Unknown,
}

/// A report-filter comparison operator.
///
/// The known Analytics operators are modeled as explicit variants; any other
/// value is preserved in [`Other`](Self::Other) so the type round-trips
/// unfamiliar operators without loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperator {
    /// `equals`
    Equals,
    /// `notEqual`
    NotEqual,
    /// `lessThan`
    LessThan,
    /// `greaterThan`
    GreaterThan,
    /// `lessOrEqual`
    LessOrEqual,
    /// `greaterOrEqual`
    GreaterOrEqual,
    /// `contains`
    Contains,
    /// `notContain`
    NotContain,
    /// `startsWith`
    StartsWith,
    /// `includes` (multi-select picklist)
    Includes,
    /// `excludes` (multi-select picklist)
    Excludes,
    /// `within` (geolocation / distance filters)
    Within,
    /// Any operator not covered by the known variants.
    Other(String),
}

impl FilterOperator {
    /// Returns the wire (camelCase) string for this operator.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Equals => "equals",
            Self::NotEqual => "notEqual",
            Self::LessThan => "lessThan",
            Self::GreaterThan => "greaterThan",
            Self::LessOrEqual => "lessOrEqual",
            Self::GreaterOrEqual => "greaterOrEqual",
            Self::Contains => "contains",
            Self::NotContain => "notContain",
            Self::StartsWith => "startsWith",
            Self::Includes => "includes",
            Self::Excludes => "excludes",
            Self::Within => "within",
            Self::Other(s) => s,
        }
    }
}

impl std::fmt::Display for FilterOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for FilterOperator {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FilterOperator {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "equals" => Self::Equals,
            "notEqual" => Self::NotEqual,
            "lessThan" => Self::LessThan,
            "greaterThan" => Self::GreaterThan,
            "lessOrEqual" => Self::LessOrEqual,
            "greaterOrEqual" => Self::GreaterOrEqual,
            "contains" => Self::Contains,
            "notContain" => Self::NotContain,
            "startsWith" => Self::StartsWith,
            "includes" => Self::Includes,
            "excludes" => Self::Excludes,
            "within" => Self::Within,
            _ => Self::Other(s),
        })
    }
}

// ── Report metadata (response + request body) ────────────────────────

/// A reference to a report type: `{ "type": ..., "label": ... }`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ReportTypeRef {
    /// The report type's API name (e.g. `"Opportunity"`).
    #[serde(rename = "type")]
    pub type_name: String,
    /// The report type's human-readable label.
    pub label: String,
}

/// A single report filter: `{ "column", "operator", "value" }`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ReportFilter {
    /// The column (field) the filter applies to.
    pub column: String,
    /// The comparison operator.
    pub operator: FilterOperator,
    /// The filter value (always sent as a string; comma-separated for
    /// multi-value operators like `includes`/`excludes`).
    pub value: String,
}

impl ReportFilter {
    /// Creates a new report filter.
    #[must_use]
    pub fn new(
        column: impl Into<String>,
        operator: FilterOperator,
        value: impl Into<String>,
    ) -> Self {
        Self {
            column: column.into(),
            operator,
            value: value.into(),
        }
    }
}

/// A grouping directive within report metadata
/// (`groupingsDown` / `groupingsAcross`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingInfo {
    /// The grouping column API name.
    pub name: String,
    /// Sort order (`"Asc"` / `"Desc"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,
    /// Date granularity for date groupings (e.g. `"Month"`, `"None"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_granularity: Option<String>,
}

/// A standard date filter on report metadata.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardDateFilter {
    /// The date column the filter applies to.
    pub column: String,
    /// The named duration (e.g. `"CURRENT_Q"`, `"CUSTOM"`).
    pub duration_value: String,
    /// Start date (`YYYY-MM-DD`), present for custom durations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// End date (`YYYY-MM-DD`), present for custom durations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

/// Report metadata.
///
/// This type is used both as a **response** (the saved definition returned by a
/// report run or `describe`) and as a **request** body component (overriding
/// filters, groupings, scope, etc. at run time). All fields are optional or
/// default-able so a partial metadata override serializes cleanly, and
/// [`Default`] enables ergonomic ad-hoc construction.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMetadata {
    /// The report Id (absent for an ad-hoc query).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The report name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The report's developer (API) name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_name: Option<String>,
    /// The report type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_type: Option<ReportTypeRef>,
    /// The report format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_format: Option<ReportFormat>,
    /// The report's filters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_filters: Vec<ReportFilter>,
    /// Custom filter logic (e.g. `"(1 OR 2) AND 3"`); `null` means all AND.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_boolean_filter: Option<String>,
    /// Detail (record-level) column API names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail_columns: Vec<String>,
    /// Summary/aggregate identifiers (e.g. `"s!Amount"`, `"RowCount"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregates: Vec<String>,
    /// Row groupings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupings_down: Vec<GroupingInfo>,
    /// Column groupings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupings_across: Vec<GroupingInfo>,
    /// Sort directives (shape varies; kept as raw JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<serde_json::Value>,
    /// The standard date filter, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_date_filter: Option<StandardDateFilter>,
    /// Standard filters (status, probability, ...); shape varies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_filters: Option<serde_json::Value>,
    /// Report scope (e.g. `"organization"`, `"mine"`, `"team"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Chart configuration, if the report has a chart; shape varies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<serde_json::Value>,
    /// ISO currency code, or `null`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Whether detail rows are included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_detail_rows: Option<bool>,
    /// Whether the record count is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_record_count: Option<bool>,
    /// Whether the grand total is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_grand_total: Option<bool>,
    /// Whether subtotals are shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_subtotals: Option<bool>,
    /// Any additional metadata fields not modeled above.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Request-body wrapper for run-with-metadata and query endpoints:
/// `{ "reportMetadata": { ... } }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMetadataRequest {
    /// The report metadata to run with.
    pub report_metadata: ReportMetadata,
}

impl ReportMetadataRequest {
    /// Wraps a [`ReportMetadata`] into a request body.
    #[must_use]
    pub const fn new(report_metadata: ReportMetadata) -> Self {
        Self { report_metadata }
    }
}

impl From<ReportMetadata> for ReportMetadataRequest {
    fn from(report_metadata: ReportMetadata) -> Self {
        Self { report_metadata }
    }
}

// ── Report run results ───────────────────────────────────────────────

/// The `attributes` block of a report-run result.
///
/// For a synchronous run this carries report identity; for an async instance
/// result it may also carry instance status fields. All fields are optional.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportAttributes {
    /// The entity type, e.g. `"Report"`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// The report Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// The report name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_name: Option<String>,
    /// URL of the report's describe resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describe_url: Option<String>,
    /// URL of the report's instances resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instances_url: Option<String>,
    /// The instance Id (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The instance status (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<InstanceStatus>,
    /// The instance request timestamp (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_date: Option<String>,
    /// The instance completion timestamp (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_date: Option<String>,
    /// The owning user's Id (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// Whether the instance is queryable (async results only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queryable: Option<bool>,
    /// Any additional attribute fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single summarized measure value in a fact-map aggregate.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SummaryValue {
    /// The formatted display label (e.g. `"$1,500,000"`).
    pub label: String,
    /// The raw value (number, `null`, or other); may be absent.
    #[serde(default)]
    pub value: serde_json::Value,
}

/// A single detail-row cell.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DataCell {
    /// The formatted display label.
    pub label: String,
    /// The raw, column-typed value.
    #[serde(default)]
    pub value: serde_json::Value,
}

/// A detail row within a fact-map entry.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    /// One cell per `detailColumns`, in order.
    #[serde(default)]
    pub data_cells: Vec<DataCell>,
}

/// A single fact-map entry keyed by a `"<down>!<across>"` coordinate.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct FactMapEntry {
    /// One aggregate per `aggregates` identifier, in order.
    #[serde(default)]
    pub aggregates: Vec<SummaryValue>,
    /// Detail rows (empty unless the run included details).
    #[serde(default)]
    pub rows: Vec<ReportRow>,
}

/// A grouping node (recursive) within `groupingsDown` / `groupingsAcross`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct Grouping {
    /// The axis index used in fact-map keys (e.g. `"0"`, `"1_0"`).
    #[serde(default)]
    pub key: String,
    /// The formatted display value.
    #[serde(default)]
    pub label: String,
    /// The raw grouping value (string/number/date/null).
    #[serde(default)]
    pub value: serde_json::Value,
    /// Nested sub-groupings (empty at a leaf).
    #[serde(default)]
    pub groupings: Vec<Self>,
    /// Date-interval sub-groupings, present only for date groupings.
    #[serde(
        rename = "dategroupings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub date_groupings: Option<serde_json::Value>,
}

/// Wrapper object for a groupings axis: `{ "groupings": [ ... ] }`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct GroupingsContainer {
    /// The top-level groupings on this axis.
    #[serde(default)]
    pub groupings: Vec<Grouping>,
}

/// Extended metadata describing the columns of a report result.
///
/// The inner descriptor objects vary by column data type and API version, so
/// they are kept as [`serde_json::Value`].
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportExtendedMetadata {
    /// Detail column descriptors, keyed by column API name.
    #[serde(default)]
    pub detail_column_info: HashMap<String, serde_json::Value>,
    /// Aggregate column descriptors, keyed by aggregate identifier.
    #[serde(default)]
    pub aggregate_column_info: HashMap<String, serde_json::Value>,
    /// Grouping column descriptors, keyed by grouping column API name.
    #[serde(default)]
    pub grouping_column_info: HashMap<String, serde_json::Value>,
}

/// The full result of running a report (sync or a completed async instance).
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResults {
    /// Report/instance identity attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<ReportAttributes>,
    /// Whether all rows were returned (`false` = capped like an in-app run).
    #[serde(default)]
    pub all_data: bool,
    /// The fact map, keyed by grouping-coordinate strings (e.g. `"T!T"`).
    #[serde(default)]
    pub fact_map: HashMap<String, FactMapEntry>,
    /// Column groupings.
    #[serde(default)]
    pub groupings_across: GroupingsContainer,
    /// Row groupings.
    #[serde(default)]
    pub groupings_down: GroupingsContainer,
    /// Whether detail rows are present.
    #[serde(default)]
    pub has_detail_rows: bool,
    /// The report metadata used for this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_metadata: Option<ReportMetadata>,
    /// Extended (column) metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_extended_metadata: Option<ReportExtendedMetadata>,
    /// Present for tabular runs that hit the row limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_exceeded_tabular_row_limit: Option<bool>,
    /// The instance status when this is a pending/running async instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<InstanceStatus>,
    /// Any additional top-level fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Instances ────────────────────────────────────────────────────────

/// A summary of an asynchronous report instance.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportInstance {
    /// The instance Id.
    pub id: String,
    /// The instance status.
    pub status: InstanceStatus,
    /// When the run was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_date: Option<String>,
    /// When the run completed (absent while pending).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_date: Option<String>,
    /// The owning user's Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// Whether the instance's results can be queried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queryable: Option<bool>,
    /// Whether the instance has detail rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_detail_rows: Option<bool>,
    /// The instance results URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Any additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Describe / listing ───────────────────────────────────────────────

/// The result of describing a report (or a report type).
///
/// `reportTypeMetadata` is deeply nested and version-variable, so it is kept as
/// raw JSON.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDescribe {
    /// The report (or template) metadata.
    pub report_metadata: ReportMetadata,
    /// The report-type metadata (categories, columns, filter operators).
    #[serde(default)]
    pub report_type_metadata: serde_json::Value,
    /// Extended (column) metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_extended_metadata: Option<ReportExtendedMetadata>,
}

/// A lightweight report descriptor from the report list endpoint.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportListItem {
    /// The report Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The report name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The report resource URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The report's describe resource URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describe_url: Option<String>,
    /// The report's instances resource URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instances_url: Option<String>,
    /// Any additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single report type within a category.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTypeInfo {
    /// The report type's API name.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// The report type's label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The report type's describe resource URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describe_url: Option<String>,
    /// Whether this type supports the joined report format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_joined_format: Option<bool>,
    /// Whether this type is hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_hidden: Option<bool>,
    /// Whether this type is available to the running user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_available: Option<bool>,
    /// Any additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A category of report types.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTypeCategory {
    /// The category label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The report types in this category.
    #[serde(default)]
    pub report_types: Vec<ReportTypeInfo>,
}

// ── Dashboards ───────────────────────────────────────────────────────

/// A lightweight dashboard descriptor from the dashboard list endpoint.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardListItem {
    /// The dashboard Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The dashboard name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The dashboard resource URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The resource type (e.g. `"Dashboard"`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub dashboard_type: Option<String>,
    /// Any additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The last-cached results of a dashboard.
///
/// Per-component data varies widely by dashboard type and API version, so
/// `componentData` entries are kept as raw JSON.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResults {
    /// The dashboard Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The dashboard name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Dashboard-level attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
    /// One raw entry per dashboard component.
    #[serde(default)]
    pub component_data: Vec<serde_json::Value>,
    /// Echoed layout/definition metadata, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_metadata: Option<serde_json::Value>,
    /// Any additional top-level fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The refresh status of a single dashboard component.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    /// The component Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    /// The component refresh status (`New`/`Running`/`Success`/`Error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// An error message if the component failed to refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Any additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The refresh status of a dashboard (from `status` or a `PUT` refresh).
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStatus {
    /// The dashboard Id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_id: Option<String>,
    /// When the refresh was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_date: Option<String>,
    /// Per-component refresh statuses.
    #[serde(default)]
    pub component_status: Vec<ComponentStatus>,
    /// Echoed dashboard metadata, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_metadata: Option<serde_json::Value>,
    /// Any additional top-level fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::must::Must;

    #[test]
    fn test_report_format_deserialize() {
        let f: ReportFormat = serde_json::from_str(r#""SUMMARY""#).must();
        assert_eq!(f, ReportFormat::Summary);
        let m: ReportFormat = serde_json::from_str(r#""MATRIX""#).must();
        assert_eq!(m, ReportFormat::Matrix);
        let mb: ReportFormat = serde_json::from_str(r#""MULTI_BLOCK""#).must();
        assert_eq!(mb, ReportFormat::MultiBlock);
        let unknown: ReportFormat = serde_json::from_str(r#""JOINED_WEIRD""#).must();
        assert_eq!(unknown, ReportFormat::Unknown);
    }

    #[test]
    fn test_report_format_serialize() {
        assert_eq!(
            serde_json::to_string(&ReportFormat::Tabular).must(),
            r#""TABULAR""#
        );
        assert_eq!(
            serde_json::to_string(&ReportFormat::MultiBlock).must(),
            r#""MULTI_BLOCK""#
        );
    }

    #[test]
    fn test_instance_status_deserialize() {
        assert_eq!(
            serde_json::from_str::<InstanceStatus>(r#""New""#).must(),
            InstanceStatus::New
        );
        assert_eq!(
            serde_json::from_str::<InstanceStatus>(r#""Success""#).must(),
            InstanceStatus::Success
        );
        assert_eq!(
            serde_json::from_str::<InstanceStatus>(r#""Something""#).must(),
            InstanceStatus::Unknown
        );
    }

    #[test]
    fn test_filter_operator_roundtrip() {
        let op: FilterOperator = serde_json::from_str(r#""notEqual""#).must();
        assert_eq!(op, FilterOperator::NotEqual);
        assert_eq!(serde_json::to_string(&op).must(), r#""notEqual""#);
        assert_eq!(op.as_str(), "notEqual");
    }

    #[test]
    fn test_filter_operator_other_preserves_value() {
        let op: FilterOperator = serde_json::from_str(r#""fuzzyMatch""#).must();
        assert_eq!(op, FilterOperator::Other("fuzzyMatch".to_string()));
        assert_eq!(serde_json::to_string(&op).must(), r#""fuzzyMatch""#);
        assert_eq!(op.to_string(), "fuzzyMatch");
    }

    #[test]
    fn test_report_metadata_request_from_metadata() {
        let meta = ReportMetadata {
            report_filters: vec![ReportFilter::new(
                "StageName",
                FilterOperator::NotEqual,
                "Closed Lost",
            )],
            ..Default::default()
        };
        let req: ReportMetadataRequest = meta.into();
        let json = serde_json::to_value(&req).must();
        assert_eq!(
            json["reportMetadata"]["reportFilters"][0]["column"],
            "StageName"
        );
        assert_eq!(
            json["reportMetadata"]["reportFilters"][0]["operator"],
            "notEqual"
        );
        // Empty/None fields must be skipped so partial overrides stay small.
        assert!(json["reportMetadata"].get("id").is_none());
        assert!(json["reportMetadata"].get("detailColumns").is_none());
    }

    #[test]
    fn test_report_results_deserialize_full_example() {
        // Verbatim SUMMARY example from the research doc (abbreviated).
        let json = serde_json::json!({
            "attributes": {
                "describeUrl": "/services/data/v67.0/analytics/reports/00O3000000B5Yn2/describe",
                "instancesUrl": "/services/data/v67.0/analytics/reports/00O3000000B5Yn2/instances",
                "reportId": "00O3000000B5Yn2",
                "reportName": "Opportunities by Stage",
                "type": "Report"
            },
            "allData": true,
            "hasDetailRows": true,
            "reportMetadata": {
                "id": "00O3000000B5Yn2",
                "name": "Opportunities by Stage",
                "developerName": "Opportunities_by_Stage",
                "reportType": { "type": "Opportunity", "label": "Opportunities" },
                "reportFormat": "SUMMARY",
                "reportFilters": [
                    { "column": "StageName", "operator": "notEqual", "value": "Closed Lost" }
                ],
                "reportBooleanFilter": null,
                "detailColumns": ["OPPORTUNITY_NAME", "AMOUNT"],
                "aggregates": ["s!AMOUNT", "RowCount"],
                "groupingsDown": [
                    { "name": "STAGE_NAME", "sortOrder": "Asc", "dateGranularity": "None" }
                ],
                "groupingsAcross": [],
                "scope": "organization",
                "currency": "USD",
                "hasDetailRows": true,
                "showGrandTotal": true,
                "showSubtotals": true
            },
            "reportExtendedMetadata": {
                "detailColumnInfo": {
                    "AMOUNT": { "label": "Amount", "dataType": "currency" }
                },
                "aggregateColumnInfo": {
                    "s!AMOUNT": { "label": "Sum of Amount", "dataType": "currency" }
                },
                "groupingColumnInfo": {
                    "STAGE_NAME": { "label": "Stage", "dataType": "picklist" }
                }
            },
            "groupingsDown": {
                "groupings": [
                    { "key": "0", "label": "Prospecting", "value": "Prospecting", "groupings": [] }
                ]
            },
            "groupingsAcross": { "groupings": [] },
            "factMap": {
                "0!T": {
                    "aggregates": [
                        { "label": "$1,500,000", "value": 1_500_000 },
                        { "label": "3", "value": 3 }
                    ],
                    "rows": [
                        { "dataCells": [
                            { "label": "Acme - 200 Widgets", "value": "Acme - 200 Widgets" },
                            { "label": "$500,000", "value": 500_000 }
                        ] }
                    ]
                },
                "T!T": {
                    "aggregates": [
                        { "label": "$1,500,000", "value": 1_500_000 },
                        { "label": "3", "value": 3 }
                    ],
                    "rows": []
                }
            }
        });

        let results: ReportResults = serde_json::from_value(json).must();
        assert!(results.all_data);
        assert!(results.has_detail_rows);

        // Attributes.
        let attrs = results.attributes.as_ref().must();
        assert_eq!(attrs.report_id.as_deref(), Some("00O3000000B5Yn2"));
        assert_eq!(attrs.type_name.as_deref(), Some("Report"));

        // Report metadata + enum.
        let meta = results.report_metadata.as_ref().must();
        assert_eq!(meta.report_format, Some(ReportFormat::Summary));
        assert_eq!(meta.report_filters[0].operator, FilterOperator::NotEqual);

        // Fact map: grand total key present, aggregate value load-bearing.
        assert!(results.fact_map.contains_key("T!T"));
        let entry = &results.fact_map["0!T"];
        assert_eq!(entry.aggregates[0].value, serde_json::json!(1_500_000));
        assert_eq!(entry.rows.len(), 1);
        assert_eq!(
            entry.rows[0].data_cells[1].value,
            serde_json::json!(500_000)
        );

        // Groupings.
        assert_eq!(results.groupings_down.groupings[0].key, "0");
        assert_eq!(results.groupings_down.groupings[0].label, "Prospecting");
        assert!(results.groupings_across.groupings.is_empty());
    }

    #[test]
    fn test_fact_map_null_aggregate_value() {
        let json = serde_json::json!({
            "aggregates": [ { "label": "$0", "value": null } ],
            "rows": []
        });
        let entry: FactMapEntry = serde_json::from_value(json).must();
        assert!(entry.aggregates[0].value.is_null());
    }

    #[test]
    fn test_report_instance_deserialize() {
        let json = serde_json::json!({
            "id": "0LG000000000001",
            "status": "New",
            "requestDate": "2026-07-12T18:02:11Z",
            "completionDate": null,
            "ownerId": "005000000000001",
            "queryable": false,
            "hasDetailRows": false,
            "url": "/services/data/v67.0/analytics/reports/00O/instances/0LG"
        });
        let inst: ReportInstance = serde_json::from_value(json).must();
        assert_eq!(inst.id, "0LG000000000001");
        assert_eq!(inst.status, InstanceStatus::New);
        assert_eq!(inst.queryable, Some(false));
    }
}
