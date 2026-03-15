# 🔭 Vantage: Spec for Schema Visualizer

**Business problem:**
Salesforce schemas evolve organically and often become highly complex and tangled webs of objects and relationships. Developers, administrators, and architects struggle to comprehend this complexity, leading to inefficient queries, redundant data models, and increased risk when making structural changes. Existing metadata describes are textual and dense, making it difficult to "see" the shape and health of the object model at a glance.

**Success metric:**
Success = Ability to generate a visual, easily digestible Markdown report (including a Mermaid.js ER diagram) for any complex SObject (like `Account` or `Opportunity`) and its relationships in under 2 seconds.

👤 **User Story:**
As a Salesforce Architect, I want to automatically generate visual documentation (Markdown and ER diagrams) of my SObject schemas, so that I can quickly understand object relationships, identify bloated data models, and communicate structural changes to stakeholders without manually drawing diagrams.

✅ **Acceptance Criteria:**
- Must combine insights from schema analysis, field usage scanning, and relationship graphs into a single comprehensive report.
- Must generate valid Mermaid.js Entity-Relationship (ER) diagrams illustrating the object and its direct dependencies.
- Must output the final report in standard Markdown format.
- Must operate entirely on existing `SObjectDescribe` payloads without requiring additional API calls for the base visualization.

🚫 **Out of Scope:**
- Interactive or dynamic web-based visualizations (Phase 2).
- Reverse-engineering Apex code to find programmatic relationships.
