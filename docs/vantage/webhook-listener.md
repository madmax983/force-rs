# 🔭 Vantage: Spec for Webhook Listener

## User Story
As an Integration Developer, I want a pre-built webhook server module that can securely receive and parse Salesforce Outbound Messages or Platform Event webhooks, so that I don't have to write custom boilerplate to verify Salesforce signatures and parse payloads.

## So What?
What business problem does this solve?
Currently, integrating with Salesforce isn't just about pulling data; it's often about reacting to data changes pushed from Salesforce. Developers building inbound webhook listeners spend significant time reinventing the wheel: verifying HMAC signatures, handling Salesforce's specific XML (for Outbound Messages) or JSON formats, and managing replay IDs. By providing a native webhook listener, we reduce integration time from days to minutes and ensure secure, best-practice handling of inbound data, making force-rs a complete bidirectional tool.

## Metric Definition
Success = 100% of valid signed payloads are accepted and parsed into typed Rust structs, while 100% of invalid or unsigned payloads are rejected with HTTP 401/403. Configuration of the listener should take less than 10 lines of code.

## Gap Analysis
The current force-rs workspace offers robust outbound API clients (REST, Bulk, Pub/Sub gRPC, etc.), but entirely lacks inbound webhook support. Developers must use generic web frameworks (like axum or actix) and manually implement Salesforce's specific security and parsing logic. There is no standard library or community crate in Rust dedicated to Salesforce webhook verification.

## Acceptance Criteria
- Must provide a generic, framework-agnostic payload parser and signature verifier.
- Must support parsing legacy SOAP-based Outbound Messages and modern JSON-based webhooks.
- Must securely validate HMAC-SHA256 signatures using a provided secret.
- Must expose typed Rust structs for the parsed events.

## Out of Scope
- Providing a full HTTP server implementation (it should integrate with existing frameworks like Axum/Actix, not replace them).
- Handling the actual business logic of what to do with the event.
