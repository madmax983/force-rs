# 🔭 Vantage: Spec for Zod Schema Generator

**Business problem:**
Enterprise teams building modern web applications (e.g., React, Next.js) or Node.js backends that integrate with Salesforce often struggle to validate data at runtime. While TypeScript interfaces provide compile-time safety, they don't protect against malformed data arriving at runtime from API calls or user input. Manually creating and updating Zod schemas to match Salesforce data models is tedious, error-prone, and leads to runtime bugs when the Salesforce schema changes.

**Gap Analysis:**
Currently, developers must either write custom validation logic, manually create Zod schemas alongside their TypeScript interfaces, or rely on weak runtime checks. Existing tools in the JavaScript/TypeScript ecosystem do not natively understand Salesforce's proprietary `SObjectDescribe` metadata. We need a lightweight solution to generate strictly-typed, runtime-enforced Zod schemas directly from live metadata.

**Success metric:**
Success = Ability to generate valid, syntactically correct TypeScript code exporting a Zod schema (e.g., `export const AccountSchema = z.object({ ... })`) for a complex SObject like `Account` or `Contact` in under 1 second.

👤 **User Story:**
As a Full-Stack or Frontend Developer, I want to automatically generate Zod schemas from Salesforce object metadata, so that I can validate API payloads and form submissions at runtime without manually maintaining validation rules.

✅ **Acceptance Criteria:**
- Must map standard Salesforce field types to corresponding Zod primitives (e.g., `z.string()`, `z.boolean()`, `z.number()`).
- Must correctly handle optional (nillable) fields using `.nullable()` or `.optional()`.
- Must output the final schema definition as syntactically correct TypeScript code.
- Must sort fields alphabetically for predictable output, with the `Id` field always appearing first.

🚫 **Out of Scope:**
- Generating complex cross-field validation rules.
- Generating schemas for related lists (child relationships) in the initial phase.
