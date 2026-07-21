# Composite Graph API

## The "So What?" (Business Problem)
Salesforce integrations often require creating or updating multiple related records in a single transaction. Doing this across multiple API calls can lead to partial failures, inconsistent data, and hitting API limits. We need a way to execute complex operations transactionally.

## User Story
As a Developer, I want to execute multiple dependent record operations in a single atomic transaction, so that I can maintain data integrity without complex rollback logic.

## Metric Definition
- Success = Graph execution latency is less than executing sequential API calls for the same payload size.
- Success = 100% rollback on transaction failure within a graph.

## Gap Analysis
Currently, we support the standard REST API for single record operations and Bulk API for large datasets, but there is no built-in mechanism for transactional multi-record operations. Standard libraries in other ecosystems provide composite or graph wrappers to handle this.

## Acceptance Criteria
- Must support defining a series of operations (create, update, delete) on related records.
- Must ensure that operations within a graph are executed transactionally.
- Must return detailed error messages for the specific operation that caused a rollback.
- Must support referencing the output of one operation as the input for a subsequent operation within the same graph.

## Out of Scope
- Visual drag-and-drop graph builders.
- Support for operations outside the scope of the standard REST Composite Graph endpoints.
