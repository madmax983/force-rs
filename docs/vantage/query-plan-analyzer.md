# 🔭 Vantage: Spec for Query Plan Analyzer — configurable cost threshold

> **Status:** `force::api::rest::analyze_query_plan` shipped — it consumes an
> `ExplainResponse`, flags `TableScan` operations, surfaces plan notes, and
> warns on high relative cost; see the crate's rustdoc for
> `force::api::rest::query_plan_analyzer`. What's below is the part of the
> original spec that did **not** ship: a caller-configurable cost threshold.
> `analyze_query_plan` hard-codes the warning trigger at
> `plan.relative_cost > 1.0` with no parameter to change it.

**Business problem:**
Not every org or query pattern treats `relative_cost > 1.0` as the right
warning line — a team running mostly large-object reports may want a higher
bar, and a team optimizing a hot path may want a lower one. Today that
threshold can't be changed without forking the analyzer.

**Gap Analysis:**
`analyze_query_plan` (`crates/force/src/api/rest/query_plan_analyzer.rs`)
takes only an `&ExplainResponse` and compares `plan.relative_cost` against
the literal `1.0`. There is no way to pass a different threshold in.

👤 **User Story:**
As a Backend Developer, I want to set my own relative-cost warning
threshold, so that `analyze_query_plan` matches what "too expensive" means
for my queries instead of a fixed default.

✅ **Acceptance Criteria:**
- Must accept a configurable relative-cost threshold (defaulting to `1.0`
  to preserve today's behavior for existing callers).
- Must not change the `TableScan` detection or `notes` surfacing that
  already ships.
- Must not require a second network round-trip; the threshold is applied to
  the `ExplainResponse` the caller already has.

🚫 **Out of Scope:**
- Configurable thresholds for anything other than `relative_cost` (e.g., a
  configurable cardinality bound) unless a future gap analysis shows a real
  need.
- Automatically rewriting the SOQL query to be more efficient.
