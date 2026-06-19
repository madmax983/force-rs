# 🔭 Vantage: Spec for Data Anonymizer

**Business problem:**
When testing Salesforce integrations, teams often need to replicate production data to lower environments (sandboxes) to debug issues or run performance tests. However, production data contains Personally Identifiable Information (PII), such as emails, phone numbers, and Social Security Numbers, which cannot legally or ethically be stored in developer sandboxes. Teams need a way to reliably mask or anonymize this data while preserving its shape and referential integrity during the seeding process.

**Success metric:**
Success = Ability to mask 1 million records across 5 distinct objects with no PII leakage in under 5 minutes, while preserving valid foreign key relationships.

👤 **User Story:**
As a QA Engineer or Developer, I want to automatically anonymize PII fields when seeding data from production to a sandbox, so that I can perform realistic testing without violating data privacy regulations.

✅ **Acceptance Criteria:**
- Must support declarative rules for anonymization (e.g., replace `Email` with fake emails, replace `Phone` with fake numbers).
- Must maintain consistent mapping (e.g., the same source email always maps to the same fake email) to preserve unique constraints and searchability.
- Must integrate seamlessly with the existing data seeding workflows.

🚫 **Out of Scope:**
- Automatically detecting PII fields within the Salesforce schema (users must explicitly map fields to anonymize).
