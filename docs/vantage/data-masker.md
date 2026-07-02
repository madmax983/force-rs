# Spec: Data Masker

👤 **User Story:** As a Compliance Officer, I want to ensure all PII data is masked when copying records to sandboxes, so that we do not violate GDPR or CCPA.

✅ **Acceptance Criteria:**
- Must identify and mask standard PII fields automatically.
- Must preserve referential integrity of the data.

🚫 **Out of Scope:** Masking data directly in production environments.

## Business Problem (The "So What?")
When synchronizing or seeding data from Salesforce Production into lower environments (Sandboxes), organizations inadvertently copy Personally Identifiable Information (PII) such as emails and phone numbers. This exposes the business to severe compliance risks (GDPR, CCPA). We need a way to reliably anonymize this data in-flight without breaking data relationships.

## Gap Analysis
- Existing tools like Salesforce Data Mask are expensive add-ons and run natively within Salesforce, consuming org resources.
- Standard libraries don't natively understand Salesforce schema types (e.g., preserving valid email formats for Salesforce validation rules).
- Current utilities like data-seeder lack a mechanism to transform data securely before insertion.

## Success Metric
- 100% of defined PII fields are transformed before reaching the target environment.
- Processing latency overhead is less than 5ms per record.