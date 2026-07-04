# Local SOQL Emulator

## Business Problem (The 'So What?')
Salesforce developers spend a significant amount of time waiting for network calls to test SOQL queries against scratch orgs or sandboxes. This network latency slows down the TDD (Test-Driven Development) loop and increases CI build times. An offline SOQL emulator will allow developers to run queries locally against a mock database, drastically reducing test execution time and dependency on external environments.

## Gap Analysis
Currently, testing SOQL requires a live connection to a Salesforce org or using heavy mocking frameworks that require manually wiring up expected responses. There is no lightweight, local SQLite-backed or in-memory SOQL engine that parses and executes SOQL syntax natively in Rust.

## Success Metric
- Query latency < 5ms for 99% of local test queries.
- 90% reduction in CI execution time for data-heavy unit tests.

## User Story
As a Developer, I want to run SOQL queries against a local, in-memory datastore, so that I can write and execute automated tests rapidly without a live Salesforce connection.

## Acceptance Criteria
- Must parse standard SOQL SELECT, FROM, WHERE, and LIMIT clauses.
- Must execute queries against an in-memory SQLite database or equivalent.
- Must return standard QueryResult structures identical to the live REST API.

## Out of Scope
- Full SOQL coverage (e.g., polymorphic relationships, complex aggregates) in Phase 1.
- DML operations (INSERT, UPDATE, DELETE) via SOQL syntax.
- Real-time synchronization with a live Salesforce org.
