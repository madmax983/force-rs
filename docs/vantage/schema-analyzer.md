# 🔭 Vantage: Spec for Schema Analyzer — zombie-field & org-wide report

> **Status:** `force::schema::analyze_schema` shipped — given one
> `SObjectDescribe`, it returns `SchemaInsights` with field counts by
> category and a structural complexity score; see the crate's rustdoc for
> `force::schema::schema_analyzer`. What's below is the part of the original
> spec that did **not** ship: identifying rarely-populated "zombie" fields,
> and processing many objects into one org-level report. `SchemaInsights` is
> structural only — it has no usage data — and `analyze_schema` takes a
> single describe, not a batch.

**What business problem does this solve?**
`analyze_schema` tells you an object is complex; it doesn't tell you which
of its fields are actually dead weight, and it can't be pointed at an org
and asked "which objects are the worst offenders?" without the caller
writing that orchestration themselves.

**Gap Analysis:**
- Zombie-field detection needs real usage data (population percentages),
  which `force::schema::scanner::FieldUsageScanner` already provides — but
  `analyze_schema` doesn't call it or fold its results into
  `SchemaInsights`, so a caller gets two disconnected reports instead of one.
- There is no `analyze_schema`-adjacent function that takes multiple
  `SObjectDescribe`s (or drives describes for an org) and returns a combined
  health report; every call is one object at a time.

👤 **User Story:**
As a Salesforce Architect, I want one report that combines an object's
structural complexity with its actual zombie fields, and that I can run
across every object in my org, so that I don't have to manually cross-
reference two separate tools per object.

✅ **Acceptance Criteria:**
- Must combine `SchemaInsights` with `FieldUsageScanner` results (where
  available) to flag fields that are both structurally present and rarely
  populated.
- Must provide a way to run the combined analysis across a list of
  `SObjectDescribe`s and produce one aggregate report, ranked by complexity
  or zombie-field count.
- Must process an org with 500+ custom objects and return the aggregate
  report in under 60 seconds (excluding the network time of fetching
  describes/usage data, which the caller controls).

🚫 **Out of Scope:**
- Automatically deleting or modifying fields in Salesforce (read-only
  analysis).
- Analyzing Apex code or Flows for field usage dependencies.
