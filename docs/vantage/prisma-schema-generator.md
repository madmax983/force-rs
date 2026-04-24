# 🔭 Vantage: Spec for Prisma Schema Generator

## 👤 User Story
As a Full-Stack Developer, I want to automatically generate a Prisma schema (`schema.prisma`) from my Salesforce org's metadata, so that I can use the Prisma ORM to interact with Salesforce data in my Node.js/TypeScript applications with full type safety and auto-completion.

## The "So What?"
**What business problem does this solve?**
Developers building web applications (e.g., using Next.js or Express) often use Prisma as their ORM of choice due to its excellent developer experience and type safety. However, manually mapping Salesforce's massive and complex data model to Prisma models is tedious, error-prone, and impossible to keep in sync as the Salesforce schema evolves. By providing a tool to generate Prisma schemas directly from Salesforce metadata, we eliminate this mapping overhead, bridging the gap between modern full-stack web development and the Salesforce platform. This dramatically reduces the time-to-market for building custom portals or applications backed by Salesforce data.

## 📈 Success Metrics
- **Success =** Given a valid Salesforce authentication, the generator produces a syntactically correct `schema.prisma` file representing 100% of the requested SObjects in under 5 seconds per 100 objects.
- **Adoption:** >20% of developers using `force-rs` for Node.js backend integration adopt the Prisma generator within the first year.
- **Accuracy:** Zero validation errors when running `prisma format` or `prisma generate` on the output file.

## Gap Analysis
Currently, developers building Node.js applications against Salesforce either write raw SOQL queries (losing type safety), manually maintain TypeScript interfaces (high maintenance burden), or use generic GraphQL. There is no automated, officially supported way to turn a Salesforce org into a set of native Prisma models. While we have tools for Zod and TypeScript interface generation, Prisma offers a complete database toolkit (ORM, migrations, studio) that many modern web frameworks assume is present.

## ✅ Acceptance Criteria
- **Model Generation:** Must translate Salesforce SObjects (e.g., `Account`, `Contact`) into Prisma `model` blocks.
- **Type Mapping:** Must accurately map Salesforce field types (e.g., `String`, `Int`, `Double`, `Boolean`, `DateTime`) to their Prisma equivalents.
- **Relationship Mapping:** Must correctly interpret Salesforce relationships (Lookup, Master-Detail) and generate Prisma relation fields (`@relation`), including resolving the correct foreign key names and reference types.
- **Nullability:** Must accurately reflect the `nillable` property of Salesforce fields as optional fields (`?`) in Prisma.
- **Custom Object Support:** Must support standard objects, custom objects (`__c`), and custom fields, properly handling the `__c` and `__r` suffixes in relationships.
- **Output Format:** Must output a valid Prisma schema file (`.prisma`) complete with a `generator` block (e.g., `provider = "prisma-client-js"`).

## 🚫 Out of Scope
- Actually implementing a Prisma connector that executes queries against the Salesforce API. This spec is strictly for generating the *schema* definitions. (Executing queries using the schema requires a separate Prisma Database Provider for Salesforce, which is not part of this tool).
- Running Prisma migrations (`prisma migrate`) against Salesforce, as Salesforce's schema is managed internally, not via Prisma DDL.
