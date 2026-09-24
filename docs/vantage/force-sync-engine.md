# 🔭 Vantage: Spec for force-sync Engine — Composite Graph apply lane

> **Status:** the `force-sync` crate shipped — a Postgres-first, durable sync
> engine with capture, planning, apply, reconcile, and recovery entry points,
> external-ID identity, and crash-safe resume; released since v0.1.0 and
> expanded through v0.4.0 (see `CHANGELOG.md`). What's below is the part of
> the original spec that did **not** ship: the Composite Graph apply lane.
> The planner already routes dependent-record workloads to it, but the
> runtime has no implementation for that lane yet.

**What business problem does this solve?**
Records with dependencies (e.g., an Opportunity and its OpportunityLineItems
created together) need to be applied as a graph so that child records can
reference a parent's not-yet-committed Salesforce ID within the same
round-trip. Without that lane, dependent-record sync tasks fail outright
instead of applying.

**Gap Analysis:**
`choose_lane` (`crates/force-sync/src/plan.rs`) returns
`ApplyLane::CompositeGraph` whenever `context.has_dependencies` is true, but
`apply_task`'s `CompositeGraph` arm (`crates/force-sync/src/runtime.rs`)
immediately calls `fail_task_for_worker` with `"unsupported runtime lane"`
and returns `Ok(false)` — the lane is selected but never executed.

👤 **User Story:**
As a Data Engineer syncing related Salesforce objects, I want dependent-record
tasks to apply via the Composite Graph API, so that parent/child records sync
together instead of every dependent task failing.

✅ **Acceptance Criteria:**
- Must implement the `CompositeGraph` arm of `apply_task` using
  `force::api::composite::graph::CompositeGraphRequest` (feature
  `composite_graph`) to submit dependent records in one graph request.
- Must resolve child record references to the parent's Salesforce ID from
  the graph response before recording success.
- Must report partial-graph failures with the same per-record fidelity the
  `Rest` and `Bulk` lanes already provide, not a single generic failure.

🚫 **Out of Scope:**
- Graphs deeper than the two-level parent/child case `choose_lane` currently
  detects.
- SQLite backend support (deferred to later phases, per the original spec).
