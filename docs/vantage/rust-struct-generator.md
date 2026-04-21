# 🔭 Vantage: Spec for Rust Model Generator

**Business problem:**
Writing Rust code to interface with the Salesforce API requires defining models for each SObject (e.g., Account, Contact). Manually writing these models is error-prone, tedious, and time-consuming, especially for objects with hundreds of custom fields. When the Salesforce schema changes, developers have to manually update their Rust models, leading to synchronization issues and broken integrations.

**Gap Analysis:**
Currently, developers either write custom translation scripts or manually type out the model definitions. Existing tools in the Rust ecosystem do not natively understand Salesforce's proprietary `SObjectDescribe` metadata. We need a lightweight, built-in solution that leverages existing metadata API calls to instantly output standard Rust code.

**Success metric:**
Success = Ability to generate valid, syntactically correct Rust code for a complex SObject like `Account` or `Contact` in under 1 second.

👤 **User Story:**
As a Rust Developer, I want to automatically generate Rust model definitions from Salesforce object metadata, so that I can quickly build compile-time safe data models without manually writing boilerplate code.

✅ **Acceptance Criteria:**
- Must map standard Salesforce field types to standard Rust types (e.g., `String`, `bool`, `i64`, `f64`).
- Must correctly map Salesforce API names to standard Rust fields.
- Must wrap nillable fields  to ensure type safety.
- Must output the final model definition as syntactically correct Rust code.

🚫 **Out of Scope:**
- Automatically saving the generated code to the local file system (should output as a string for Phase 1).
- Automatically generating comprehensive builder patterns.
