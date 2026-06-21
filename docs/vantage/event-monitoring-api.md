# 🔭 Vantage: Spec for Event Monitoring API

## Business Problem (The "So What?")
Enterprise security and compliance teams must continuously audit Salesforce org activity, such as who exported reports, API usage patterns, and login forensics. Currently, teams must manually query the EventLogFile object and handle raw binary or CSV streams, making it difficult to integrate Salesforce audit logs into SIEM systems without building custom data pipelines. Providing a dedicated API surface simplifies compliance and security monitoring out of the box.

## Gap Analysis
The standard REST capability can query standard objects, but it does not natively handle the secondary process of downloading the large, base64-encoded or raw CSV payload of the actual log file. Developers currently have to manually extract the log file path, inject bearer tokens into raw HTTP requests, and manage memory constraints when dealing with multi-gigabyte log files. We lack a dedicated capability that streams and parses these log files efficiently.

## Success Metric
Success = Ability to stream and process a 1GB event log file into parsed records without exceeding 50MB of memory overhead, and seamlessly integrate into processing pipelines.

## User Story
👤 **User Story:**
As a Security Engineer, I want a dedicated Event Monitoring capability to automatically stream and parse Salesforce Event Log Files, so that I can ingest audit data into my systems without building custom polling and memory-managed HTTP streaming clients.

## Acceptance Criteria
✅ **Acceptance Criteria:**
- Must provide an intuitive way to retrieve available event logs by date and event type.
- Must stream the log data over HTTP directly to the caller or provide an iterator to process records lazily.
- Must automatically handle the standard formats provided by the Salesforce Event Monitoring feature.
- Must reuse the existing HTTP connection pool and authentication lifecycle.

## Out of Scope
🚫 **Out of Scope:**
- Automatic SIEM integration or forwarding (e.g., Datadog/Splunk clients).
- Processing of real-time events (Real-time Event Monitoring uses Pub/Sub, which is separate).