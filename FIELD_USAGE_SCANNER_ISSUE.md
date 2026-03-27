# 🔭 Vantage: Spec for Field Usage Scanner

**Business problem:**
Over time, Salesforce schemas accumulate custom fields. Many of these fields are created for one-off projects or become obsolete as business processes change, resulting in "zombie fields" that are rarely or never populated. These unused fields clutter the page layout, confuse users, increase metadata complexity, and can negatively impact API and SOQL query performance. Administrators lack an automated way to statistically analyze field utilization across millions of records.

**Gap Analysis:**
Existing field usage tools in Salesforce (like Field Trip) require installing managed packages, storing results in custom objects, and dealing with batch apex limits. We need a lightweight, API-driven utility that can instantly scan an SObject and return utilization percentages without installing any code in the target org.

**Success metric:**
Success = Ability to execute a field usage scan on an SObject (like `Account`) with up to 100 fields, correctly batching SOQL aggregate queries (e.g., 20 fields per query), and return a list of `FieldUsage` statistics in under 3 seconds.

👤 **User Story:**
As a Salesforce Administrator or Architect, I want an automated tool to scan an SObject and report the exact population percentage for every field, so that I can confidently identify and deprecate "zombie fields" to reduce schema bloat.

✅ **Acceptance Criteria:**
- Must first fetch the `SObjectDescribe` to determine all available fields.
- Must filter out unscanable fields (e.g., `Address`, `Location`, `Base64`, `Encryptedstring`).
- Must automatically batch SOQL `COUNT()` aggregate queries into chunks of 20 fields to avoid exceeding Salesforce SOQL character limits.
- Must calculate and return the total record count, populated record count, and utilization percentage (`0.0 - 100.0`) for each scanable field.
- Must safely handle cases where the SObject has zero records (avoiding divide-by-zero or NaN percentages).

🚫 **Out of Scope:**
- Automatically deleting or modifying the identified "zombie fields" in Salesforce (Phase 2).
- Scanning sub-queries or nested child relationship fields.
