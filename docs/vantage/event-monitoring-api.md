# Event Monitoring API Spec

## The "So What?"
Salesforce administrators and security teams need visibility into user activity, API usage, and potential security threats within their orgs. Without native programmatic access to the Salesforce EventLogFile data, security teams cannot effectively integrate Salesforce logs into their centralized SIEM (Security Information and Event Management) systems for threat detection and compliance auditing. Providing a dedicated API to query and download these event logs in a structured format bridges this critical visibility gap.

## User Story
As a Security Engineer, I want to programmatically query, download, and parse EventLogFile records in a structured format, so that I can seamlessly forward Salesforce security events to our SIEM for threat detection and compliance auditing.

## Metric Definition
- **Success:** Ability to extract and parse 1GB+ EventLogFile records with constant memory overhead (stream processing).
- **Latency:** Query and pagination overhead for log discovery should complete in < 500ms.
- **Robustness:** 100% of standard event types can be downloaded without encountering structural panics.

## Gap Analysis
- **Current State:** Users must manually query the `EventLogFile` sObject via SOQL, handle base64 decoding of large CSV/JSON payloads, and manually map the dynamic schema of over 50 different event types.
- **Standard Libs / Alternatives:** Some users write bespoke Python scripts or use heavy middleware. There is no native, typed Rust API for seamlessly streaming and parsing these logs efficiently.

## Acceptance Criteria
- Must provide an API to discover EventLogFile records based on EventType, CreatedDate, or Interval (Hourly/Daily).
- Must include a utility to download and decode the LogFile payload as a stream to avoid OOM errors on large logs.
- Must provide structural mappings (or generic JSON/Map fallback) for common event types.
- Must handle API rate limits appropriately during large log extractions.

## Out of Scope
- Real-time event streaming (Event Monitoring via Real-Time Event Monitoring/PubSub is a separate capability).
- Direct integrations with specific SIEMs (e.g., a Splunk exporter). The API should just provide the structured data.
- Modifying or deleting EventLogFiles (they are inherently read-only).
