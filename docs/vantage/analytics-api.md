# 🔭 Vantage: Spec for Analytics API

## Overview
A specification to implement support for the Salesforce Reports and Dashboards REST API (Analytics API) within force-rs.

## 👤 User Story
As a Data Engineer, I want to execute existing Salesforce reports programmatically, so that I can extract pre-calculated metrics, aggregations, and filtered data into our data warehouse without having to reverse-engineer and maintain complex SOQL queries.

## The "So What?"
**What business problem does this solve?**
Business users often define complex logic, aggregations, and groupings directly in Salesforce Reports. Replicating these reports via SOQL is difficult, fragile, and creates dual-maintenance. By supporting the Analytics API natively, we allow downstream systems to execute these reports directly and consume the final aggregated results. This bridges the gap between CRM business logic and external data pipelines, saving engineering time and preventing metric drift.

## 📈 Success Metrics
- **Success =** Ability to execute a synchronous report and retrieve tabular data within standard API timeouts.
- **Adoption:** Top requested feature among enterprise customers migrating from legacy data extraction tools.
- **Reliability:** 100% correct parsing of report metadata, fact maps, and aggregated values.

## Gap Analysis
The crate currently provides robust ways to extract raw data via SOQL (REST, Bulk, and GraphQL). However, it lacks any native abstraction for the Analytics API. Developers trying to fetch report data must manually construct REST operations and parse the extremely nested JSON response (factMap, groupingsDown, groupingsAcross), which is notoriously complex and error-prone.

## ✅ Acceptance Criteria
- **Execution Support:** Must support synchronous execution of tabular, summary, and matrix reports via the `/services/data/vX.X/analytics/reports/{reportId}` endpoint.
- **Metadata Parsing:** Must clearly map the returned metadata (columns, data types) to structured types.
- **Fact Map Deserialization:** Must flatten or cleanly expose the factMap and groupings into a usable, iterable data structure to hide the underlying JSON complexity.
- **Filtering:** Must allow overriding report filters dynamically at runtime.

## 🚫 Out of Scope
- Support for asynchronous report execution (which requires polling).
- Support for creating, modifying, or deleting reports and dashboards.
- Dashboard execution or metadata retrieval.
