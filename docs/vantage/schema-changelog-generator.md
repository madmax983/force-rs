# 🔭 Vantage: Spec for Schema Changelog Generator

**Business problem:**
Salesforce schemas change over time as new fields are added, types are modified, or fields are removed. Tracking these changes manually across deployments or between environments is difficult and error-prone. Administrators and Developers lack an automated way to generate readable changelogs, leading to confusion during release cycles and a lack of historical context for schema evolution.

**Gap Analysis:**
Currently, developers must manually compare XML files or use heavy metadata diffing tools to identify changes between schema versions. There is no lightweight, built-in solution that leverages existing `SObjectDescribe` payloads to instantly output a human-readable Markdown changelog tracking added, removed, and modified fields.

**Success metric:**
Success = Ability to generate a valid, Markdown-formatted changelog comparing two `SObjectDescribe` payloads in under 1 second.

👤 **User Story:**
As a Salesforce Release Manager or Architect, I want to automatically generate a Markdown changelog comparing two SObject schema versions, so that I can easily document and communicate data model changes during deployments without manual diffing.

✅ **Acceptance Criteria:**
- Must compare two `SObjectDescribe` payloads using the `compare_schemas` utility.
- Must clearly list "Added Fields", "Removed Fields", and "Changed Fields" with their API names, labels, and types.
- Must output the final report in standard Markdown format.
- Must operate entirely on existing in-memory describe payloads without requiring additional API calls.

🚫 **Out of Scope:**
- Automatically committing the changelog to a version control system (Phase 2).
- Comparing Apex code or other metadata types (strictly focused on SObjects).
