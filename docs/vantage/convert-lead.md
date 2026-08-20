# 🔭 Vantage: Spec for Convert Lead

**Business problem:**
When a lead is qualified, sales teams need to convert it into an Account, Contact, and optionally an Opportunity. Doing this manually via standard CRUD API calls is tedious, error-prone, and loses native Salesforce lead conversion semantics (like mapping rules and conversion audit trails). We need a dedicated operation that leverages the native Salesforce Lead Conversion API to ensure data integrity and trigger correct downstream automation.

**Gap Analysis:**
The REST API currently provides basic CRUD, but converting a lead requires a specialized endpoint or SOAP API equivalent. Without native support for Lead Conversion in `force-rs`, developers have to manually orchestrate complex composite requests which don't trigger native conversion logic correctly.

**Success metric:**
Success = Ability to convert a Lead into an Account and Contact (and optionally an Opportunity) in a single API call with native conversion logic, returning the IDs of the newly created records.

👤 **User Story:**
As a CRM Integration Developer, I want a dedicated `convert_lead` API function, so that I can programmatically qualify and convert leads while fully respecting our org's native Lead Mapping rules and triggering appropriate automation.

✅ **Acceptance Criteria:**
- Must provide a strongly-typed input structure for lead conversion parameters.
- Must support optionally creating an Opportunity during conversion.
- Must return the newly generated `accountId`, `contactId`, and optionally `opportunityId`.
- Must handle batch lead conversion to stay within API limits.

🚫 **Out of Scope:**
- Configuring the Lead Mapping rules themselves (this is done in Salesforce Setup).
- Lead deduplication logic beyond what the native conversion API handles.
