# Local SOQL Emulator

## The "So What?" (Business Problem)
Developers building integrations with Salesforce often spend significant time and face friction testing SOQL queries. They either have to write mock classes, connect to a live sandbox (which may have data consistency issues or network latency), or risk pushing untested queries to higher environments. This creates a slow developer loop and increases the cost of building robust Salesforce integrations. A local emulator allows developers to run automated tests and validate queries instantly, increasing development velocity and reducing the load on actual Salesforce orgs.

## User Story
👤 **User Story:** As an Engineer, I want a local emulator for SOQL queries, so that I can run automated tests and validate queries without needing a live Salesforce org connection.

## Metric Definition
- **Success:** >95% of standard SOQL queries execute successfully against the local emulator in under 5ms, without requiring an external HTTP request.
- **Adoption:** 30% of new projects using `force-rs` incorporate the local emulator in their automated test suites within 6 months of release.

## Gap Analysis
- **Current State:** Developers must mock network responses, maintain complex test setups, or rely on live API calls to test SOQL functionality. There is no standard, fast way to validate SOQL syntax and execution logic offline in the Rust ecosystem.
- **Market Alternatives:** Some tools offer partial mocking, and Salesforce has its own testing framework in Apex, but for external applications (like those built with `force-rs`), developers lack a lightweight, native, and fast SOQL evaluation engine.

## Acceptance Criteria
✅ **Acceptance Criteria:**
- Must support basic SOQL `SELECT` statements locally (including `WHERE`, `LIMIT`, and `ORDER BY`).
- Must provide feedback on invalid query syntax (e.g., malformed queries).
- Must operate entirely offline with no org connection required.
- Must allow developers to load a local dataset (e.g., via JSON or CSV) to query against.

## Out of Scope
🚫 **Out of Scope:**
- Execution of DML statements (Insert, Update, Delete).
- Support for Bulk API operations.
- Complex SOQL features in phase 1 (e.g., polymorphic relationships, aggregate functions like `GROUP BY ROLLUP`).
- Full replication of Salesforce's security model (Field-Level Security, Sharing Rules).
