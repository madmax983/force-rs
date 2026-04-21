# 🔭 Vantage: Spec for Streaming API

## Overview
A utility to connect and interact with Salesforce's legacy Streaming API (CometD/Bayeux) for PushTopics, Generic Events, and older Platform Events.

## User Story
As an Enterprise Systems Integrator, I want to subscribe to Salesforce PushTopics and generic events using the Streaming API, so that I can maintain backward compatibility with older Salesforce eventing architectures that have not yet migrated to the modern gRPC Pub/Sub API.

## The "So What?"
**What business problem does this solve?**
While Salesforce's new Pub/Sub API (gRPC) is the future, a vast number of legacy enterprise deployments still heavily rely on the older CometD/Bayeux-based Streaming API for PushTopics and generic events. Without native support for the Streaming API in the `force` crate, customers migrating their middleware to Rust would be forced to run hybrid stacks or manually implement the complex Bayeux protocol, Long Polling, and connection state management. Supporting this unlocks modernization for customers with legacy technical debt.

## Metric Definition
- **Success =** Ability to establish a Bayeux connection, handshake, subscribe to a PushTopic, and receive 1,000 continuous events without dropping the connection or failing to reconnect during network blips.

## Gap Analysis
The `force` crate roadmap includes the modern `pub_sub` feature (gRPC), but there is no native CometD/Bayeux client currently available in the Rust ecosystem that easily integrates with Salesforce authentication (OAuth/Session ID). Developers currently have to write raw HTTP long-polling loops, which are fragile and difficult to maintain.

## Acceptance Criteria
- Must implement the CometD/Bayeux protocol (Handshake, Connect, Subscribe, Disconnect).
- Must support Long Polling transport.
- Must support subscribing to PushTopics (`/topic/`), Generic Events (`/u/`), and standard Platform Events (`/event/`).
- Must automatically handle Bayeux message replay (`replayId`) to prevent data loss upon unexpected disconnects.
- Must seamlessly utilize the existing authentication mechanisms to inject the Session ID into the Bayeux handshake.

## Out of Scope
- Support for WebSockets (initial phase will use Long Polling for maximum compatibility).
- Managing or creating PushTopics programmatically (should be managed via Metadata API/UI).
