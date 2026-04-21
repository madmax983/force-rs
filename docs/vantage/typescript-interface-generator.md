# 🔭 Vantage: Spec for TypeScript Interface Generator

**Business problem:**
Enterprise teams building modern web applications (e.g., React, Angular, Vue) or Node.js backends that integrate with Salesforce often struggle to keep their frontend TypeScript types synchronized with the actual Salesforce schema. Manually creating and updating these interfaces is tedious, error-prone, and leads to runtime bugs when custom fields are added, removed, or modified in Salesforce.

**Gap Analysis:**
Currently, developers must either rely on generic `any` types, write custom scripts to parse metadata, or manually copy-paste schema changes into their `.ts` files. There is no lightweight, out-of-the-box solution to generate strictly-typed TypeScript interfaces directly from live `SObjectDescribe` payloads.

**Success metric:**
Success = Ability to generate a valid, syntactically correct TypeScript interface (e.g., `export interface Account { ... }`) for a complex SObject like `Account` or `Contact` with 100+ fields in under 1 second.

👤 **User Story:**
As a Full-Stack or Frontend Developer, I want to automatically generate TypeScript interfaces from Salesforce object metadata, so that I can have compile-time safety when interacting with Salesforce data and avoid manually maintaining type definitions.

✅ **Acceptance Criteria:**
- Must map standard Salesforce field types to standard TypeScript types (e.g., `boolean`, `number`, `string`).
- Must correctly represent optional (nillable) fields.
- Must mark read-only (non-updateable) fields with the `@readonly` JSDoc tag.
- Must include field labels and inline help text as JSDoc comments above each property.
- Must sort fields alphabetically for predictable output, with the `Id` field always appearing first.

🚫 **Out of Scope:**
- Automatically generating comprehensive API client functions (e.g., fetch methods) alongside the interfaces.
- Generating types for related lists (child relationships) in the initial phase.
