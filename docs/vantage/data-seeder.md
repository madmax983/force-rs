# 🔭 Vantage: Spec for Data Seeder

**Business problem:**
Testing Salesforce integrations often requires realistic records populated in a sandbox or scratch org. Developers and QA engineers spend excessive time writing custom scripts, using DataLoader, or manually creating records via the UI to establish test data states. This slows down testing cycles and makes it difficult to reliably recreate complex data environments across CI/CD pipelines.

**Gap Analysis:**
Existing tools like DataLoader require manual CSV preparation and mapping. Apex scripts require deployment and are difficult to maintain. The currently available DataFaker generates in-memory mock data but doesn't persist it. There is a missing link to seamlessly generate and insert hundreds of valid, mock records directly into an org using existing metadata and batch APIs.

**Success metric:**
Success = Ability to generate and successfully insert 500 valid, schema-compliant records for a standard object like `Account` into a Salesforce org in under 5 seconds using the Composite Batch API.

👤 **User Story:**
As a Salesforce Developer or QA Engineer, I want a tool to automatically generate and seed a specified number of mock records directly into my sandbox, so that I can quickly establish realistic data states for testing without manual data entry or CSV management.

✅ **Acceptance Criteria:**
- Must leverage the `DataFaker` to generate schema-compliant mock data based on live `SObjectDescribe` metadata.
- Must efficiently insert records into Salesforce using the Composite Batch API to minimize API calls and avoid rate limits.
- Must handle batch limits automatically (e.g., chunking requests if the count exceeds the maximum allowed subrequests per composite batch).
- Must provide clear success/failure reporting, including the number of records successfully inserted and any Salesforce API errors encountered.
- Must support an option to halt processing immediately if a batch operation fails .

🚫 **Out of Scope:**
- Seeding complex relational data trees (e.g., Accounts with related Contacts and Opportunities) in a single operation (Phase 2).
- Automatic deletion or rollback of seeded data after tests complete (Phase 2).
