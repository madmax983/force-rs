# 🔭 Vantage: Spec for Query Plan Analyzer

**Business problem:**
Salesforce REST API SOQL queries can become massive performance bottlenecks or trigger timeouts if they lack proper indexing or involve full table scans. Developers currently lack a built-in mechanism to automatically analyze the performance implications and cost of their queries before execution, leading to degraded application performance and poor user experiences.

**Gap Analysis:**
The Salesforce REST API provides a Query Explain endpoint (`/services/data/vXX.X/query/?explain=...`) which returns raw cost and execution plan data. However, there is no automated, developer-friendly way to validate this output against configurable thresholds (e.g., throwing a warning if `relative_cost > 1.0` or if `leading_operation_type` is `TableScan`). Developers are left to interpret these JSON payloads manually.

**Success metric:**
Success = Ability to execute an `explain()` analysis on a SOQL query and produce an actionable evaluation report (pass/warning) based on configurable cost thresholds in under 20ms.

👤 **User Story:**
As a Backend Developer, I want to automatically analyze the execution plan of my SOQL queries before they run in production, so that I can detect and prevent inefficient table scans and high-cost operations.

✅ **Acceptance Criteria:**
- Must expose an analyzer module that consumes a `QueryPlan` response payload.
- Must identify non-indexed queries (e.g., `TableScan`).
- Must flag query plans with a `relative_cost` that exceeds a configurable threshold (e.g., `relative_cost > 1.0`).
- Must output actionable warnings and context from the plan's `notes` property.

🚫 **Out of Scope:**
- Automatically rewriting the SOQL query to be more efficient.
- Applying index modifications directly to the Salesforce org.
