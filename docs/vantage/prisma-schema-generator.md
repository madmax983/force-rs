# 🔭 Vantage: Spec for Prisma Schema Generator

## Overview
A utility to generate Prisma schema definitions (`.prisma`) from Salesforce `SObjectDescribe` metadata.

## User Story
As a Full-Stack Node.js/TypeScript Developer, I want to automatically convert Salesforce object metadata into a Prisma schema definition, so that I can rapidly build full-stack applications with strongly-typed ORM access to my synced Salesforce data without manually defining the schema.

## The "So What?"
**What business problem does this solve?**
Many modern web applications use Prisma ORM for database access because of its excellent TypeScript integration and developer experience. When enterprises sync Salesforce data into a local database (like PostgreSQL), developers have to manually recreate the Salesforce data model within their `schema.prisma` file. This manual translation is slow, prone to data type mismatches, and frustrating to maintain when custom fields are added in Salesforce. By automatically generating Prisma schemas from Salesforce metadata, we drastically reduce the time to value for full-stack teams building on top of synchronized Salesforce data, enabling them to focus on application logic instead of schema maintenance.

## Metric Definition
- **Success =** Generated Prisma schema files must compile successfully via `prisma format` and `prisma generate` without syntax or validation errors. Standard types should map flawlessly, and unique identifiers should be correctly marked.

## Gap Analysis
The `force::schema` module currently provides generation capabilities for various languages and technologies (e.g., Avro, GraphQL, BigQuery, Pydantic) but lacks an automated way to output a Prisma schema. While `force-sync` handles the data synchronization into Postgres, generating the Prisma schema bridges the gap for Node/TypeScript application developers querying that database.

## Acceptance Criteria
- Must map Salesforce data types (e.g., `string`, `int`, `boolean`, `double`, `datetime`, `date`) to Prisma types (`String`, `Int`, `Boolean`, `Float`, `DateTime`).
- Must handle the Salesforce `nillable` attribute by making the Prisma field optional (`?`).
- Must appropriately map the Salesforce `Id` field to the Prisma `@id` attribute.
- Must handle fields marked as `unique` in Salesforce by appending the Prisma `@unique` attribute.
- Must alphabetize fields in the output for deterministic generation and easy diffing.

## Out of Scope
- Actually generating the Prisma Client library (we only generate the schema.prisma file definition).
- Handling Prisma relations (`@relation`) natively (since Salesforce relationship graphs are extremely complex and often don't map cleanly to strict foreign keys without junction tables).
