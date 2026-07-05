# Data Masking

**The Business Problem:**
When developers export production data to their local machines or lower environments (sandboxes) for debugging and testing, they risk exposing Personally Identifiable Information (PII) such as emails, phone numbers, and Social Security Numbers. A data breach from a developer's laptop can lead to severe compliance violations (GDPR, CCPA) and loss of customer trust. We need a way to automatically sanitize data as it is exported, leveraging Salesforce's schema metadata to intelligently identify and mask sensitive fields without manual configuration.

**Gap Analysis:**
Currently, developers must either manually configure static exclusion lists (which quickly become outdated as schemas evolve) or use expensive third-party sandbox seeding tools. Our standard library (`force-rs`) has a robust schema extraction mechanism (`SObjectDescribe`) but lacks a native utility to bridge schema awareness directly into a sanitization pipeline during data export (`DataArchiver`).

**Success Metric:**
- Zero unmasked PII data in exported files (JSON/CSV) when the masking utility is enabled.
- Performance impact of masking should be < 5% overhead during large Bulk API exports.

👤 **User Story:**
As a Salesforce Developer, I want to automatically mask sensitive PII fields when exporting production data, so that I can safely debug issues locally without risking compliance violations.

✅ **Acceptance Criteria:**
- The utility must automatically identify sensitive fields using heuristics (e.g., field type is Email/Phone, `encrypted` flag is true, or name matches PII patterns).
- The utility must support in-place mutation of a generic record structure (e.g., `DynamicSObject`) to avoid heavy allocation overhead.
- Must provide configurable masking strategies (e.g., replacing emails with `***@***.***`, replacing text with `[REDACTED]`).
- Must handle null values gracefully without error.

🚫 **Out of Scope:**
- Format-preserving encryption (FPE) for reversibility.
- Advanced NLP-based detection of PII within unstructured text (e.g., reading a generic `Description` field to find a hidden phone number).
- Uploading masked data back into Salesforce (this is an export sanitization utility only).
