# 🔭 Vantage: Spec for Zod Schema Generator

**Business problem:**
While TypeScript interfaces provide excellent compile-time safety for developers building applications against Salesforce, they disappear at runtime. When ingesting API responses, webhook payloads, or Pub/Sub events from Salesforce, applications need robust runtime validation to ensure data integrity and fail fast on unexpected payloads. Writing and maintaining these runtime validators manually is error-prone and falls out of sync with the live Salesforce schema.

**Gap Analysis:**
Currently, developers must either blindly trust that the incoming JSON payload perfectly matches their TypeScript interfaces (a dangerous assumption), or write tedious, custom validation boilerplate. Zod has become the industry standard for TypeScript runtime validation, but there is no native tooling to automatically bridge Salesforce `SObjectDescribe` metadata into ready-to-use Zod schemas.

**Success metric:**
Success = Ability to generate a valid, syntactically correct Zod schema (e.g., `export const AccountSchema = z.object({ ... });`) for a complex SObject like `Account` or `Contact` with 100+ fields in under 1 second.

👤 **User Story:**
As a Full-Stack or Backend Developer, I want to automatically generate Zod schemas from Salesforce object metadata, so that I can strictly validate incoming Salesforce data payloads at runtime without manually maintaining the validation logic.

✅ **Acceptance Criteria:**
- Must map standard Salesforce field types to standard Zod types (e.g., `boolean` to `z.boolean()`, `string` to `z.string()`, `double` to `z.number()`).
- Must correctly apply `.nullable()` or `.optional()` for fields where `nillable` is true.
- Must apply `.max(length)` validations for string fields based on the `length` property in the describe metadata.
- Must include JSDoc comments with field labels and help text above each schema property.
- Must sort fields alphabetically for predictable output, with the `Id` field always appearing first.

🚫 **Out of Scope:**
- Generating complex custom `.refine()` logic for cross-field validations or Salesforce validation rules.
- Generating schemas for deeply nested child relationship records in the initial phase.
