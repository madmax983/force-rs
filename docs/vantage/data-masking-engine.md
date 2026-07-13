# 🔭 Vantage: Spec for Data Masking Engine

**The 'So What?' (business problem):**
When moving production data to lower sandbox environments for testing, organizations expose sensitive PII (Personally Identifiable Information) and PHI to developers and testers. This violates compliance requirements (GDPR, HIPAA, SOC2) and increases the risk of data breaches. Manual scripts are hard to maintain and often miss fields.

**Gap Analysis:**
Standard Salesforce sandbox seeding tools either lack built-in masking or charge a premium for it (e.g., Salesforce Data Mask). Existing open-source scripts are usually slow, sequential, and tied to specific object schemas, lacking the ability to declaratively mask data based on patterns or metadata.

**Metric Definition:**
Success = Ability to mask 100,000 records across 10 objects with 5 different masking strategies (e.g., scramble, static, regex) in under 10 seconds.

👤 **User Story:**
As a Compliance Officer, I want to automatically obscure sensitive fields during data syncs to lower environments, so that developers can test with realistic data without exposing actual PII.

✅ **Acceptance Criteria:**
- Must support declarative masking rules (e.g., via YAML or JSON).
- Must provide built-in masking strategies: static replacement, random string/number generation, and format-preserving email/phone scrambling.
- Must execute masking entirely in-memory before data is written to the destination.
- Must maintain referential integrity (e.g., if a contact's email is masked, it should be masked consistently across related records if configured).

🚫 **Out of Scope:**
- Automatically detecting PII fields (users must explicitly map which fields to mask).
- Masking data already at rest in Salesforce (the engine acts on data in transit).