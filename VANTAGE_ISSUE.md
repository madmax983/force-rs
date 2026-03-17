# 🔭 Vantage: Spec for Pub/Sub API (gRPC Event Streaming)

**Business problem:**
Salesforce data changes rapidly, and polling the REST API for updates consumes org limits and introduces latency. Enterprise integrations need real-time event streaming to trigger downstream workflows (e.g., updating a local database or notifying external services) instantly when Salesforce records are created or modified.

**Success metric:**
Success = Ability to receive 1,000 events per minute with < 500ms latency without unexpected connection drops.

👤 **User Story:**
As an Enterprise Integration Engineer, I want to subscribe to Salesforce Change Data Capture (CDC) and Platform Events via a gRPC stream, so that I can process data changes in real-time without polling and hitting API limits.

✅ **Acceptance Criteria:**
- Must establish a secure, authenticated gRPC connection to the Salesforce Pub/Sub API.
- Must support subscribing to standard and custom topics (e.g., `/data/ChangeEvents`).
- Must automatically handle authentication token refreshing and connection reconnects if the stream drops.

🚫 **Out of Scope:**
- Publishing events back to Salesforce (Phase 2).
- Managing or creating new Platform Event definitions programmatically (should be managed via Metadata API/UI).

**Gap Analysis:**
Currently, developers rely on polling the REST API, which is slow and consumes significant API limits. We previously had experimental Pub/Sub support but removed it (ADR-011) due to heavy gRPC/Protobuf dependencies (`tonic`, `prost`) that complicated the build process. A dedicated crate or optional feature is required to re-introduce this without bloating the core REST/Bulk client.
