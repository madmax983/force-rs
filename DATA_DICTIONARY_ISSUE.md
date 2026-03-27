# 🔭 Vantage: Spec for Data Dictionary Generator

**What business problem does this solve?**
Business analysts and developers struggle to maintain up-to-date documentation of the Salesforce data model. Understanding what fields exist, what they mean, and whether they are actually used requires navigating complex UIs or running manual queries. Lack of clear documentation leads to duplicated effort and data quality issues.

👤 **User Story:**
As a Data Analyst or Architect, I want to automatically generate a Markdown data dictionary for any Salesforce object, so that I have an easily readable, up-to-date reference of the schema and field utilization statistics without manual documentation.

✅ **Acceptance Criteria:**
- Must generate a comprehensive Markdown document detailing the fields of an SObject based on its `SObjectDescribe` payload.
- Must optionally integrate with the `FieldUsageScanner` to include real utilization statistics (population percentages) alongside the field definitions.
- Must include field attributes like API Name, Label, Type, and Help Text.
- Success = Ability to generate a Markdown data dictionary for an object with 100+ fields, including usage stats, in under 5 seconds.

🚫 **Out of Scope:**
- Exporting to proprietary formats like PDF or Excel (Markdown is the standard output).
