# 🔭 Vantage: Spec for SQL DDL Exporter

**What business problem does this solve?**
Enterprise teams often need to replicate Salesforce data into local relational databases (like PostgreSQL or SQLite) for data warehousing, local testing, or offline analysis. Manually mapping Salesforce's proprietary `SObjectDescribe` schema to standard SQL data types is error-prone, tedious, and time-consuming, forcing developers to build custom scripts or rely on heavy ETL tools.

**Gap Analysis:**
Currently, developers either write custom translation scripts for every object or purchase expensive integration tools (e.g., FiveTran) just to obtain the schema definitions. We need a lightweight, built-in solution that leverages existing metadata API calls to instantly output standard DDL.

👤 **User Story:**
As a Data Engineer, I want to automatically generate SQL table definitions from Salesforce object metadata, so that I can easily spin up a local database mirror for offline analysis and data warehousing without manual schema mapping.

✅ **Acceptance Criteria:**
- Must map standard Salesforce field types (e.g., `string`, `reference`, `double`) to standard SQL column types (e.g., `VARCHAR(255)`, `VARCHAR(18)`, `DOUBLE PRECISION`).
- Must correctly configure the primary key (typically the `Id` field) and handle basic constraints like nullability.
- Must extract insights and generate DDL from `SObjectDescribe` payloads without requiring manual review.
- Success = Ability to generate valid, syntactically correct SQL DDL for a complex object like `Account` or `Contact` in under 1 second.

🚫 **Out of Scope:**
- Actually executing the generated SQL against a target database system.
- Data synchronization (exporting or importing the actual row data; this is strictly for DDL schema generation).
