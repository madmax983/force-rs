# 🔭 Vantage: Spec for SOQL Query Builder

**Business problem:**
Writing raw SOQL strings is brittle, error-prone, and vulnerable to injection. Developers spend excessive time debugging syntax errors, misspelled field names, and complex relationship queries. A type-safe query builder reduces development time, prevents runtime errors, and makes the codebase more maintainable.

**Success metric:**
Success = 0 runtime syntax errors for queries constructed via the builder, and a reduction in boilerplate string manipulation code.

👤 **User Story:**
As a Rust Developer, I want to construct SOQL queries using a fluent, type-safe API, so that I can catch invalid field references and syntax errors at compile-time rather than runtime.

✅ **Acceptance Criteria:**
- Must provide a fluent interface for constructing `SELECT`, `FROM`, `WHERE`, `ORDER BY`, `LIMIT`, and `OFFSET` clauses.
- Must support nested relationship queries (e.g., `SELECT Id, (SELECT Name FROM Contacts) FROM Account`).
- Must handle proper escaping of string literals and formatting of date/time literals in the `WHERE` clause.
- Must cleanly serialize the builder state into a valid SOQL string for execution by the REST handler.

🚫 **Out of Scope:**
- SOSL search query builder (this spec is strictly for SOQL).
- Automatic fetching of schema metadata to validate queries against the live org during compilation.
