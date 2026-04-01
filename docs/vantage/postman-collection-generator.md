# 🔭 Vantage: Spec for Postman Collection Generator

**Business problem:**
Developers and integration engineers frequently need to test Salesforce REST API interactions with specific objects (e.g., `Account` or `Custom_Object__c`). Creating Postman collections manually for each object is a tedious process that requires constantly cross-referencing schema definitions to know which fields are required, createable, or updateable. This slows down API discovery, prototyping, and integration testing.

**Gap Analysis:**
While Postman is the industry standard for API testing, it lacks native integration with Salesforce's dynamic object schemas. Developers currently have to manually construct JSON request bodies and endpoint URLs for every object they wish to test. We need an automated way to instantly convert live Salesforce schema metadata into a ready-to-use Postman collection.

**Success metric:**
Success = Ability to generate a valid Postman v2.1.0 Collection JSON file containing Create, Read, Update, and Delete operations for a complex SObject with 100+ fields in under 1 second.

👤 **User Story:**
As an Integration Engineer, I want to automatically generate a comprehensive Postman Collection for any Salesforce object based on its schema, so that I can quickly test REST API operations without manually authoring request payloads and URLs.

✅ **Acceptance Criteria:**
- Must generate a valid Postman v2.1.0 compatible JSON collection.
- Must include standard CRUD operations (Create, Read, Update, Delete) for the specified SObject.
- Must automatically construct Create and Update JSON request bodies populated with the object's createable and updateable fields, respectively.
- Must pre-fill request URLs with standard Salesforce REST API paths and postman variables (e.g., `{{_endpoint}}` and `{{recordId}}`).

🚫 **Out of Scope:**
- Automatically pushing the generated collection into a user's Postman workspace via the Postman API.
- Generating Postman tests or pre-request scripts within the collection (Phase 2).
