# 🔭 Vantage: Spec for SOQL Query Builder — nested relationship queries

> **Status:** `force::api::SoqlQueryBuilder` shipped — a fluent, injection-safe
> builder for `SELECT`/`FROM`/`WHERE`/`ORDER BY`/`LIMIT`/`OFFSET`, with
> `try_*` (`Result`) and panicking convenience variants; see the crate's
> rustdoc for `force::api::soql`. What's below is the part of the original
> spec that did **not** ship: nested relationship (sub-query) selects.
> Every field passed to `select`/`try_select` goes through
> `validate_field_name`, which rejects spaces and commas — so a subquery
> like `(SELECT Name FROM Contacts)` cannot be passed as a selected field.

**Business problem:**
Parent-to-child relationship queries (`SELECT Id, (SELECT Name FROM
Contacts) FROM Account`) are common SOQL, but today they can't be built
through `SoqlQueryBuilder` — callers have to drop to a raw string for any
query that needs one, losing the builder's escaping and validation for the
whole query.

**Gap Analysis:**
`SoqlQueryBuilder::select`/`try_select`
(`crates/force/src/api/soql.rs`) validates each selected field with
`validate_field_name`, which allows only alphanumerics, `_`, `.`, and
balanced parentheses around dotted/functional field paths — not the
`SELECT ... FROM ...` syntax of a child subquery.

👤 **User Story:**
As a Rust Developer, I want to add a child relationship subquery to a
`SoqlQueryBuilder` query, so that I can build parent-child queries without
abandoning the builder for a raw string.

✅ **Acceptance Criteria:**
- Must provide a way to include a child relationship subquery (e.g., a
  `select_related`/`sub_query` builder method distinct from `select`) that
  is validated as its own mini query rather than rejected as an invalid
  field name.
- Must compose with the existing top-level `WHERE`/`ORDER BY`/`LIMIT` clauses
  without changing their current behavior.
- Must preserve `try_*`/panicking method pairing already established by the
  rest of the builder.

🚫 **Out of Scope:**
- Arbitrarily deep nested subqueries beyond one level of parent-to-child.
- SOSL search query building (this spec is strictly for SOQL).
