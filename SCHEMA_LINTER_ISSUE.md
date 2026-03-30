# 🔭 Vantage: Spec for Schema Linter

**What business problem does this solve?**
Salesforce schemas often contain anti-patterns, limit violations, and unused metadata, leading to technical debt and deployment failures. Teams lack an automated way to evaluate their object schemas against best practices and limits before deploying changes.

👤 **User Story:**
As a Salesforce Architect or Release Manager, I want to automatically lint SObject schemas against best practices and limits, so that I can prevent technical debt and ensure a healthy data model.

✅ **Acceptance Criteria:**
- Must evaluate an `SObjectDescribe` against customizable lint rules.
- Must output findings with severities (`Warning` or `Info`) and clear explanatory messages.
- Must include default rules like `TooManyFieldsRule` and `MissingCustomSuffixRule`.
- Success = Ability to evaluate a complex object against a set of rules and return actionable findings in under 1 second.

🚫 **Out of Scope:**
- Automatically fixing the identified issues in the Salesforce org (read-only analysis).
