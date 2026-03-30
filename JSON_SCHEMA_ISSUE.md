# 🔭 Vantage: Spec for JSON Schema Generator

**What business problem does this solve?**
External systems and API gateways integrating with Salesforce often require standard JSON Schema definitions for data validation. Manually translating Salesforce's proprietary `SObjectDescribe` into standard JSON Schema is tedious and quickly becomes outdated as the data model changes.

👤 **User Story:**
As an Integration Engineer, I want to automatically generate standard JSON Schema (draft-07) definitions from Salesforce object metadata, so that I can define strict validation rules for external API gateways and services without manual mapping.

✅ **Acceptance Criteria:**
- Must generate a valid JSON Schema draft-07 specification from an `SObjectDescribe` payload.
- Must correctly map Salesforce field types to JSON Schema primitive types (e.g., `string`, `number`, `boolean`).
- Must accurately identify and list required fields based on the `createable`, `nillable`, and `defaulted_on_create` metadata.
- Success = Ability to output a valid JSON Schema object for a complex SObject in under 1 second.

🚫 **Out of Scope:**
- Handling nested relationship objects or sub-schemas (focus on flat object schemas for Phase 1).
