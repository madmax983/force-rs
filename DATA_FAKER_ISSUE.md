# 🔭 Vantage: Spec for Data Faker

**Business problem:**
Testing Salesforce integrations requires realistic mock data that conforms to a specific org's schema (custom fields, required fields, data types). Manually creating this data for unit and integration tests is tedious, error-prone, and brittle when schemas evolve, leading to broken tests and reduced developer velocity.

**Success metric:**
Success = Ability to automatically generate a valid, schema-compliant mock payload (e.g., `DynamicSObject`) for any given SObject describe in under 10ms.

👤 **User Story:**
As an Integration Test Engineer, I want to automatically generate mock Salesforce records based on live `SObjectDescribe` metadata, so that my local tests are resilient to schema changes and I don't have to manually maintain dummy JSON payloads.

✅ **Acceptance Criteria:**
- Must automatically generate sensible default values based on the Salesforce `FieldType` (e.g., proper formats for dates, emails, and URLs).
- Must only generate data for `createable` fields, explicitly ignoring `auto_number` and `calculated` (formula) fields.
- Must handle `Picklist` fields gracefully by selecting a valid active option if available in the metadata.
- Must gracefully ignore complex or unsupported field types (like `Base64` or `Location`) without failing the generation process.

🚫 **Out of Scope:**
- Automatically inserting the generated mock data into Salesforce (the utility should only return the in-memory payload).
- Handling complex relational data generation (e.g., automatically generating parent and child records recursively).
