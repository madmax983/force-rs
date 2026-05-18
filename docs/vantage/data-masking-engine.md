# 🔭 Vantage: Spec for Data Masking Engine

**What business problem does this solve?**
Protecting PII/PHI in lower environments (sandboxes) is critical for compliance (GDPR, CCPA, HIPAA). Moving production data to sandboxes for testing exposes sensitive customer information. Existing solutions are expensive add-ons and can be slow. We need a fast, local masking engine that integrates seamlessly with our data seeding workflows, ensuring developers have realistic but safe data.

👤 **User Story:**
As a Compliance Officer and QA Engineer, I want to mask PII/PHI data when syncing from production to sandboxes, so that developers can test with realistic data volumes and shapes without exposing sensitive customer information.

**Metric Definition:**
- Success = Ability to mask a 1,000,000 record dataset locally in < 5 minutes before pushing to a sandbox.
- Success = Zero leakage of unmasked data for configured fields.

**Gap Analysis:**
Standard Salesforce data masking is an expensive add-on. Open-source tools lack native Salesforce schema awareness or struggle with deterministic masking required to maintain relational integrity (e.g., matching external IDs across systems).

✅ **Acceptance Criteria:**
- Must support deterministic masking (same input always yields the same output) to preserve cross-object relationships.
- Must support common PII/PHI formats (emails, phone numbers, SSNs, names, addresses, credit cards).
- Must integrate directly with the existing `data-seeder` and `force-sync` configurations.
- Must support custom regex-based masking rules.

🚫 **Out of Scope:**
- Auto-discovery or auto-classification of PII fields (Phase 2).
- Real-time on-the-fly masking of production SOQL queries.
