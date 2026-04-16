# Prisma Schema Generator

## Overview
A utility to generate Prisma schema models (`schema.prisma`) from Salesforce `SObjectDescribe` metadata.

## User Story
As a Full-Stack Developer, I want to automatically generate Prisma models from my Salesforce object schemas, so that I can quickly build strongly-typed Node.js/TypeScript APIs against my Salesforce data without manual mapping.

## The "So What?"
**What business problem does this solve?**
Developers building web applications or microservices on top of Salesforce often use modern ORMs like Prisma. Manually maintaining Prisma models that match Salesforce SObject definitions is a tedious, error-prone process that slows down development and risks runtime errors when the Salesforce schema changes. By automatically generating Prisma schemas from Salesforce metadata, we accelerate full-stack development, eliminate boilerplate mapping, and guarantee type safety.

## Metric Definition
- **Success =** Generated Prisma models pass the `prisma validate` command with zero errors and accurately reflect Salesforce data types and nillable properties for all standard objects.

## Gap Analysis
While `force-rs` currently offers JSON Schema, GraphQL, Protobuf, and Pydantic schema generators, there is no built-in support for generating Prisma ORM models. Developers in the Node.js/TypeScript ecosystem rely heavily on Prisma, and lacking this generator forces them to resort to manual translation or disjointed third-party tooling.

## Acceptance Criteria
- Must map standard Salesforce data types (e.g., `boolean`, `int`, `double`, `currency`, `date`, `datetime`) to correct Prisma native types (`Boolean`, `Int`, `Float`, `DateTime`).
- Must map unhandled/string types to Prisma's `String` type.
- Must respect the `nillable` attribute (mapping to optional Prisma fields using `?`).
- Must correctly annotate primary keys (`@id` for the `Id` field).
- Must correctly annotate unique fields (`@unique` when `unique` is true).

## Out of Scope
- Actually migrating or introspecting a live database with Prisma (e.g., `prisma db pull`).
- Managing Prisma client generation or executing Prisma queries.
- Generating schemas for complex parent-child relationships beyond the SObject's base fields.
