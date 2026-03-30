# 🔭 Vantage: Spec for Streaming API (CometD/Bayeux)

**What business problem does this solve?**
While the Pub/Sub API handles modern high-volume gRPC event streaming, many legacy enterprise systems still rely on PushTopics, generic events, and CometD/Bayeux protocols for real-time updates. Without supporting the legacy Streaming API, these users are forced to either upgrade all their Salesforce infrastructure or build custom polling logic, which burns through API quotas and increases latency.

👤 **User Story:**
As a Salesforce Integration Developer maintaining existing PushTopic configurations, I want to subscribe to legacy Streaming API channels using the CometD protocol, so that I can receive real-time data changes without migrating my org to the new Pub/Sub API.

✅ **Acceptance Criteria:**
- Must establish a secure WebSocket/long-polling connection using the CometD (Bayeux) protocol.
- Must support subscribing to PushTopics, Generic Events, and standard Platform Events.
- Must automatically handle token refreshment and CometD handshakes, reconnecting seamlessly if the network drops.
- Success = Ability to sustain a persistent connection and receive events with < 1s latency on legacy orgs.

🚫 **Out of Scope:**
- High-volume data synchronization (this should be routed to Pub/Sub API).
- Creating or managing PushTopics programmatically (should be managed via Metadata API/UI).
