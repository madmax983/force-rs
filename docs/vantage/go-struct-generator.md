# 🔭 Vantage: Spec for Go Struct Generator

## Business Problem
Many enterprise systems use Go for backend microservices due to its performance and concurrency model. When integrating with Salesforce, developers spend significant time manually writing Go structs to match Salesforce object schemas, which are prone to errors and become outdated when custom fields change in Salesforce.

## Gap Analysis
- **Current State:** Developers manually create Go types for Salesforce data, leading to brittle code and runtime unmarshalling errors when Salesforce types change.
- **Alternatives:** Existing tools often target other languages (e.g., TypeScript, Python) or require complex intermediate steps.
- **The Gap:** We need a direct generation path from Salesforce SObjectDescribe metadata to idiomatic Go structs.

## Success Metric
- Developers can generate fully documented, idiomatic Go structs with correct JSON tags from Salesforce metadata without manual intervention.

## User Story
**As a Backend Engineer**, I want to automatically generate Go structs from Salesforce metadata, so that my Go microservices can safely and reliably consume Salesforce data without manually maintaining type definitions.

## Acceptance Criteria
- Must take Salesforce SObjectDescribe metadata as input.
- Must map Salesforce data types to appropriate Go types.
- Must correctly handle optional or nullable fields.
- Must generate correct struct field tags for JSON unmarshalling.
- Must include field descriptions from Salesforce metadata as Go comments.

## Out of Scope
- Direct deployment or publishing of generated Go code.
- Generation of complete Go client libraries or HTTP clients.
- Advanced relationship mapping for nested queries.