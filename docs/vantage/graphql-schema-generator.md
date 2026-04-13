# 🔭 Vantage: Spec for GraphQL Schema Generator

## Overview
A utility to generate GraphQL schema definitions (`.graphql` or SDL) from Salesforce `SObjectDescribe` metadata.

## User Story
As a Frontend Developer or API Gateway Architect, I want to automatically convert Salesforce object metadata into a GraphQL schema definition, so that I can easily integrate Salesforce data into our federated GraphQL graph or use strongly-typed GraphQL clients (like Apollo) without manually maintaining the types.

## The "So What?"
**What business problem does this solve?**
Many modern web applications and API gateways use GraphQL for its flexible, strongly-typed data fetching capabilities. However, integrating Salesforce data requires developers to manually duplicate and maintain the Salesforce object schema within their GraphQL server definitions. This manual process is tedious, error-prone, and constantly falls out of sync as administrators add or modify custom fields in Salesforce. By dynamically generating the GraphQL Schema Definition Language (SDL) directly from the Salesforce API, we eliminate manual type definitions, ensure front-end clients always have 100% accurate types, and drastically reduce the friction of integrating Salesforce into a modern composable architecture.

## Metric Definition
- **Success =** 100% of generated GraphQL schemas pass validation by standard GraphQL parsers (like `graphql-js` or Apollo Server) without syntax or type errors. The generation process should complete in under 50 milliseconds for an object with 500+ fields.

## Gap Analysis
The `force::schema` module currently provides schema generators for BigQuery, JSON Schema, Protobuf, Avro, and various programming languages, and the core crate has a `graphql` feature for *executing* queries, but it lacks a generator for the GraphQL Schema Definition Language itself. Adding this generator allows teams to bridge `force-rs` into GraphQL gateways and developer workflows that rely on SDL files for code generation.

## Acceptance Criteria
- Must map Salesforce data types (e.g., `string`, `int`, `boolean`, `double`, `id`, `reference`) to standard GraphQL scalar types (`String`, `Int`, `Boolean`, `Float`, `ID`).
- Must correctly handle the Salesforce `nillable` attribute by applying the GraphQL non-null modifier (`!`) to fields that are NOT nillable.
- Must structure the output as a valid GraphQL `type` definition.
- Must sort the generated fields alphabetically for deterministic output, ensuring easier diffing.

## Out of Scope
- Generating full GraphQL API resolvers or server implementations (this generator is only responsible for the schema definition types).
- Generating Query or Mutation root types that span multiple SObjects.
- Nested or related object type generation (e.g., generating an `Account` type with a `contacts` field that resolves to a `[Contact]`).
