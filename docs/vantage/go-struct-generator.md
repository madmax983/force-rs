# Spec: Go Struct Generator

## User Story
As a Backend Engineer, I want to automatically generate Go structs from Salesforce object metadata, so that I can build type-safe microservices that interact with Salesforce data without manual, error-prone transcription.

## So What?
**What business problem does this solve?**
Translating Salesforce objects (which can have hundreds of fields) into Go structs is tedious and error-prone. Providing an automated generator accelerates integration development, ensures type safety, and reduces bugs caused by schema mismatches in Go-based downstream systems.

## Metric Definition
**Success =**
- Time-to-first-query in Go reduced from hours to minutes.
- 99% of standard and custom Salesforce fields are correctly mapped to standard Go types.
- 0 compile-time errors in the generated Go code.

## Gap Analysis
Currently, force-rs can generate Rust, Python (Pydantic), and TypeScript types, but completely ignores the Go ecosystem, which is highly prevalent in enterprise backend and microservices architectures. Developers currently have to write these structs by hand or rely on third-party, unmaintained tools.

## Acceptance Criteria
- Must generate valid Go struct definitions from a DescribeSObjectResult.
- Must include standard json struct tags for serialization.
- Must map Salesforce data types to appropriate Go types.
- Must handle optional/nullable fields by generating pointers.
- Must generate correct time types for Date and Datetime fields.

## Out of Scope
- Complete ORM integration.
- Generating client implementation code or HTTP request builders.
- Support for generating Go interfaces or mock structs.
