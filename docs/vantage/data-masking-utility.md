# 🔭 Vantage: Spec for Data Masking Utility

**Business Problem (The 'So What?'):**
Testing and debugging with production Salesforce data poses a significant security and compliance risk (e.g., exposing PII or violating GDPR/CCPA). Manually scrubbing exported data is time-consuming and error-prone, slowing down developer workflows and data science initiatives.

**Gap Analysis:**
Standard libraries and existing ETL tools either require complex configuration or lack native understanding of Salesforce schema (e.g., preserving valid email formats, maintaining picklist constraints, or understanding field-level encryption). We need a utility that can autonomously mask data payloads while maintaining valid Salesforce structure.

**Success Metric:**
Success = Ability to mask a payload of 10,000 Salesforce records with 50+ fields in under 5 seconds, ensuring no original PII is identifiable while maintaining schema-compliant data types.

👤 **User Story:**
As a Data Engineer, I want an automated utility to mask sensitive fields in Salesforce data payloads, so that I can safely load production-like data into sandbox environments or data lakes without violating security policies.

✅ **Acceptance Criteria:**
- Must provide configurable masking strategies (e.g., redaction, shuffling, random generation) based on data types.
- Must preserve referential integrity when masking foreign keys or external IDs if configured to do so.
- Must output the masked data in a format identical to the input payload structure.
- Must execute entirely in memory without writing unmasked data to disk.

🚫 **Out of Scope:**
- Automatic detection of PII fields; masking targets must be explicitly configured.
- Direct execution of masking operations within the live Salesforce org (this is an off-platform utility).
