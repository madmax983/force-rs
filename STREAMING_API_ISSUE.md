# 🔭 Vantage: Spec for Streaming API (CometD/Bayeux)

**What business problem does this solve?**
Legacy Salesforce streaming features (PushTopic, generic events) still rely on the CometD/Bayeux protocol rather than the newer gRPC Pub/Sub API. Many organizations have existing integrations built on these legacy events that need ongoing support without migrating to gRPC.

👤 **User Story:**
As an Enterprise Integration Engineer, I want to subscribe to Salesforce PushTopics and legacy streaming events via a CometD/Bayeux client, so that I can maintain compatibility with existing streaming infrastructure without migrating to the Pub/Sub API.

✅ **Acceptance Criteria:**
- Must establish an authenticated connection using the CometD/Bayeux protocol over WebSockets or long polling.
- Must support subscribing to legacy topics (e.g., `/topic/InvoiceStatements`).
- Must handle connection state, including handshakes, connects, disconnects, and automatic reconnections with exponential backoff.
- Success = Ability to reliably receive events from a PushTopic with < 500ms latency and reconnect automatically after network interruptions.

🚫 **Out of Scope:**
- Publishing events back to Salesforce via CometD (Read-only subscriptions for Phase 1).
- Managing or creating PushTopics programmatically.
