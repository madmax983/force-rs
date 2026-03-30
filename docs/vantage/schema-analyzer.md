# 🔭 Vantage: Spec for Schema Analyzer

**What business problem does this solve?**
Salesforce schemas evolve rapidly, leading to accumulated technical debt, "zombie" fields, and complex, inefficient objects that impact API performance and system maintainability. Administrators and Developers lack automated tools to quantify object complexity, identify unused custom fields, and understand the true cost of their data model, leading to bloated data payloads and slow queries.

👤 **User Story:**
As a Salesforce Architect or Administrator, I want a tool to automatically analyze and extract insights from SObject describe metadata, so that I can identify schema bloat, uncover unused fields, and calculate object complexity metrics.

✅ **Acceptance Criteria:**
- Must calculate a complexity score for SObjects based on the number of fields, relationships, and custom configurations.
- Must identify potential "zombie" fields (e.g., custom fields that are rarely populated or unused in page layouts/queries).
- Must extract actionable insights from `SObjectDescribe` payloads without requiring manual review.
- Success = Ability to process an org with 500+ custom objects and output a schema health report in under 60 seconds.

🚫 **Out of Scope:**
- Automatically deleting or modifying fields in Salesforce (Read-only analysis for Phase 1).
- Analyzing Apex code or flows for field usage dependencies.
