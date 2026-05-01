# 🔭 Vantage: Spec for DBML Schema Generator

## Overview
A utility to generate Database Markup Language (DBML) representations from Salesforce `SObjectDescribe` metadata.

## User Story
As a Database Architect, I want to automatically convert Salesforce object metadata into DBML, so that I can easily visualize and document our Salesforce data model using tools like dbdiagram.io without manual data entry.

## The "So What?"
**What business problem does this solve?**
Salesforce schemas are often massive and complex, making it difficult for new engineers and data teams to understand the relationships between objects. Standard Salesforce ERD tools are locked inside the platform and hard to share. DBML is an open-source, code-driven format for defining and documenting database schemas and relations. Generating DBML directly from the source of truth allows teams to maintain version-controlled visual documentation and share ERDs seamlessly across the organization.

## Metric Definition
- **Success =** Generated DBML files must be 100% compliant with the DBML specification and render correctly without errors in dbdiagram.io or dbdocs.io.

## Gap Analysis
While we have tools for generating SQL DDL or code-level types, we lack a dedicated format strictly for generating visual entity-relationship diagrams (ERDs) that are version-controllable and platform agnostic.

## Acceptance Criteria
- Must map Salesforce data types to DBML column definitions.
- Must mark required (non-nillable) fields with the `not null` setting.
- Must add `note` settings for field labels and inline help text if present.
- Must define foreign key relationships based on Salesforce `referenceTo` metadata.

## Out of Scope
- Generating SVG or PDF diagrams directly.
- Bi-directional sync (updating Salesforce from DBML).
