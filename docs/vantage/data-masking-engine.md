# 🔭 Vantage: Spec for Data Masking Engine

**Business problem:**
Enterprise Salesforce orgs contain sensitive Personally Identifiable Information (PII), Protected Health Information (PHI), and financial data. When deploying data to sandboxes for development or testing, this sensitive data is often inadvertently exposed to developers and contractors, risking compliance violations (GDPR, CCPA, HIPAA). Existing solutions are often expensive third-party managed packages or manual scripts that are difficult to maintain and scale across massive datasets.

**Gap Analysis:**
The `force` crate currently provides `data-faker` (for generating purely synthetic data) and `data-archiver` (for exporting data), but lacks an automated, configurable engine to fetch existing records, irreversibly mask or anonymize specific fields (e.g., replacing real emails with fakes, masking SSNs, shuffling names), and write them back or export them safely.

**Success metric:**
Success = Ability to execute a data masking job over a 1-million record `Contact` dataset, applying deterministic or random masking rules to 5 distinct fields, with a throughput of at least 5000 records/second, without exposing any original sensitive data in the output.

👤 **User Story:**
As a Salesforce Administrator or Compliance Officer, I want an automated tool to mask sensitive fields in sandbox data, so that I can provide realistic datasets to my development team without violating data privacy regulations or risking data breaches.

✅ **Acceptance Criteria:**
- Must provide a configurable masking rule engine (e.g., `RandomEmail`, `DeterministicString`, `Shuffle`, `StaticValue`, `Redact`).
- Must integrate with the Bulk API 2.0 to efficiently extract and update massive datasets.
- Must ensure that deterministic masking (e.g., mapping `john.doe@example.com` to `fake123@mask.com`) produces consistent results across multiple runs if configured with the same seed.
- Must provide a dry-run mode that outputs a sample of masked data for verification without modifying the source org.

🚫 **Out of Scope:**
- Automatic discovery or classification of sensitive fields (e.g., PII detection algorithms) (Phase 2).
- Masking of unstructured data within Attachments, Files, or rich text fields (Phase 2).
