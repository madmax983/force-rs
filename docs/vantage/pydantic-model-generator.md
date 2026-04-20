# 🔭 Vantage: Spec for Pydantic Model Generator

## Business Problem
Many enterprise data science and data engineering teams rely on Python to process Salesforce data (e.g., using FastAPI, Airflow, or Pandas). Maintaining synchronization between Salesforce's dynamic schema (custom fields, changing data types) and the Python codebase is a manual, error-prone process. When a Salesforce administrator adds a new field or changes a type, downstream Python applications often break due to missing or mismatched data validation layers.

## Gap Analysis
- **Current State:** Teams manually write and maintain Pydantic models, TypedDicts, or Dataclasses to represent Salesforce objects. This results in stale types and runtime validation errors.
- **Alternatives:** Existing tools often only generate simple dictionaries or are tightly coupled to specific ORMs (like Django or SQLAlchemy), lacking robust, standalone runtime validation schemas.
- **The Gap:** We need an automated way to translate Salesforce `SObjectDescribe` metadata directly into robust, self-validating Python Pydantic models (v2) that can be easily integrated into modern Python stacks.

## Success Metric
- Teams can generate complete Pydantic models for standard and custom objects without manual intervention.
- The generated code must be syntactically valid Python and correctly utilize Pydantic v2 features (e.g., `Field`, `ConfigDict`).

## User Story
**As a Data Engineer**, I want to generate Pydantic models directly from Salesforce metadata, so that my Python data pipelines and APIs have strongly typed, auto-updating validation schemas that mirror our Salesforce org.

## Acceptance Criteria
- Must take Salesforce `SObjectDescribe` metadata as input.
- Must map Salesforce data types (e.g., `string`, `int`, `boolean`, `datetime`, `double`, `percent`, `currency`) to appropriate Python types (`str`, `int`, `bool`, `datetime`, `float`).
- Must correctly handle optional/nullable fields .
- Must include field-level documentation/descriptions extracted from the Salesforce metadata as Pydantic `Field` descriptions.
- Must correctly handle Salesforce field name semantics, particularly mapping API names like `My_Custom_Field__c` cleanly.
- Must generate models that are compatible with Pydantic v2.

## Out of Scope
- Direct deployment of these models to Python package registries (e.g., PyPI).
- Generation of complete SQLAlchemy or Django ORM models.
- Handling complex nested relational queries (e.g., mapping deep child-to-parent relationships in a single generated model, beyond standard reference fields).
