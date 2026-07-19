# 🔭 Vantage: Spec for Nested Relationship Records

**The 'So What?' (business problem):**
Creating a parent record and its associated child records currently requires multiple round-trip API calls. This increases latency, consumes valuable Salesforce API limits, and requires manual transaction rollback logic if a child record creation fails after the parent was created. A nested relationship payload allows inserting a parent and its children in a single atomic transaction.

**User Story:**
As an Integration Developer, I want to create a parent record and its child records in a single API call, so that I can reduce API limit usage and ensure data consistency without manual rollbacks.

**Metric Definition:**
Success = Ability to insert a parent record and up to 50 child records in a single API request, resulting in exactly 1 API call consumed.

**Gap Analysis:**
The current REST API handler (`create` / `create_typed`) only supports flat JSON structures for a single sObject. To create children, developers must wait for the parent ID and issue subsequent requests. Other standard libraries (like JSForce) support nested creation out-of-the-box. We are missing this parity.

**Acceptance Criteria:**
- Must support serializing a nested JSON payload where child records are attached to the parent under the correct relationship name.
- Must provide a strongly-typed way to model nested creations in the client language.
- Must handle Salesforce's specific JSON structure requirements for nested inserts (e.g., including the `attributes` block with `type`).

**Out of Scope:**
- Composite Graph API implementation (which handles more complex, multi-level dependency graphs and unrelated objects).
- Deeply nested records beyond 1 level of parent-child (Phase 2).
